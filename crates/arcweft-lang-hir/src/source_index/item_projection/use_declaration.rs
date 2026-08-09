//! Exact source projection for flattened final-HIR use bindings.

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::source_file::{
    AttachedPath, AttachedUseAlias, AttachedUseGroupChild, AttachedUseTree,
};
use arcweft_lang_syntax::attachment::{AstNode, TypedItemNode, node::UseDeclarationKind};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_source::SourceSpan;

use crate::identity::{ItemId, SyntheticOwner};
use crate::item::{HirItemKind, HirUseBinding, HirUseBindingKind, HirUseDeclaration};
use crate::source_index::expression_manifest::leaf::{
    path_projection_matches, path_projection_with_terminal_matches,
};
use crate::source_index::{
    HirItemSourceRole, HirSourceCommitInvariantError, HirSourceIndex, HirSourceQuery,
    HirSourceQueryError, HirSourceRequirement, HirSourceSite, HirUseBindingSourcePart,
    HirUseSourceRole, StagedHirSourceIndex,
};

#[derive(Default)]
struct UseManifest {
    requirements: BTreeMap<HirSourceQuery, HirSourceRequirement>,
    components: BTreeMap<HirSourceQuery, HirSourceSite>,
}

impl UseManifest {
    #[allow(
        clippy::result_large_err,
        reason = "required Use rows preserve complete typed query and source evidence"
    )]
    fn required(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        ordinal: u32,
        part: HirUseBindingSourcePart,
        span: SourceSpan,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.insert(
            parsed,
            use_binding_query(owner, ordinal, part),
            HirSourceRequirement::Required,
            Some(span),
        )
    }

    #[allow(
        clippy::result_large_err,
        reason = "optional Use rows preserve complete typed query and source evidence"
    )]
    fn optional(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        ordinal: u32,
        part: HirUseBindingSourcePart,
        span: Option<SourceSpan>,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.insert(
            parsed,
            use_binding_query(owner, ordinal, part),
            HirSourceRequirement::Optional,
            span,
        )
    }

    #[allow(
        clippy::result_large_err,
        reason = "Use manifest insertion preserves complete typed query and source evidence"
    )]
    fn insert(
        &mut self,
        parsed: &ParsedSource,
        query: HirSourceQuery,
        requirement: HirSourceRequirement,
        span: Option<SourceSpan>,
    ) -> Result<(), HirSourceCommitInvariantError> {
        if self
            .requirements
            .insert(query.clone(), requirement)
            .is_some()
        {
            return Err(HirSourceCommitInvariantError::ConflictingRequirement { query });
        }
        if let Some(span) = span {
            let site = HirSourceSite::from_attached_span(parsed.document(), &span)?;
            if self.components.insert(query.clone(), site).is_some() {
                return Err(HirSourceCommitInvariantError::ConflictingComponent { query });
            }
        } else if requirement == HirSourceRequirement::Required {
            return Err(HirSourceCommitInvariantError::MissingRequiredComponent { query });
        }
        Ok(())
    }
}

impl StagedHirSourceIndex {
    /// Stages one Use declaration's flattened binding/source correspondence.
    #[allow(
        clippy::result_large_err,
        reason = "Use staging preserves complete typed owner and manifest evidence"
    )]
    pub(crate) fn stage_attached_use(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        attached: &TypedItemNode,
        retained: &HirItemKind,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        let (TypedItemNode::Use(attached), HirItemKind::Use(retained)) = (attached, retained)
        else {
            if matches!(attached, TypedItemNode::Use(_)) || matches!(retained, HirItemKind::Use(_))
            {
                return self.reject(
                    HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                        owner: SyntheticOwner::Item(owner),
                    },
                );
            }
            return Ok(());
        };
        if attached.snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.snapshot_id().clone(),
            });
        }
        if !payload_matches(attached, retained) {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                    owner: SyntheticOwner::Item(owner),
                },
            );
        }
        let manifest = match use_manifest(parsed, owner, attached, retained) {
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

impl HirUseDeclaration {
    pub(crate) fn validate_use_source_role(
        &self,
        owner: ItemId,
        role: HirUseSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        match role {
            HirUseSourceRole::Whole => Ok(()),
            HirUseSourceRole::Binding { ordinal, .. } => {
                let in_bounds = usize::try_from(ordinal)
                    .ok()
                    .is_some_and(|ordinal| ordinal < self.bindings().len());
                if in_bounds {
                    Ok(())
                } else {
                    Err(HirSourceQueryError::ItemOrdinalOutOfBounds {
                        owner,
                        role: HirItemSourceRole::Use(role),
                        length: u32::try_from(self.bindings().len()).unwrap_or(u32::MAX),
                    })
                }
            }
        }
    }
}

pub(super) fn payload_matches(
    attached: &AstNode<UseDeclarationKind>,
    retained: &HirUseDeclaration,
) -> bool {
    let Ok(tree) = attached.tree() else {
        return false;
    };
    match &tree {
        AttachedUseTree::Path { path, alias } => matches!(retained.bindings(), [binding]
            if binding.kind() == HirUseBindingKind::Item
                && path_projection_matches(binding.path(), path)
                && alias_matches(binding, alias.as_ref())),
        AttachedUseTree::Glob { module, alias, .. } => matches!(retained.bindings(), [binding]
            if binding.kind() == HirUseBindingKind::Glob
                && path_projection_matches(binding.path(), module)
                && alias_matches(binding, alias.as_ref())),
        AttachedUseTree::Group {
            module, children, ..
        } => {
            let bindings = children.iter().filter_map(|child| match child {
                AttachedUseGroupChild::Binding(binding) => Some(binding),
                AttachedUseGroupChild::Recovery { .. } => None,
            });
            retained.bindings().len() == bindings.clone().count()
                && retained
                    .bindings()
                    .iter()
                    .zip(bindings)
                    .all(|(retained, attached)| {
                        retained.kind() == HirUseBindingKind::Item
                            && path_projection_with_terminal_matches(
                                retained.path(),
                                module,
                                attached.kind(),
                                attached.name().source_text(),
                            )
                            && alias_matches(retained, attached.alias())
                    })
        }
    }
}

pub(super) fn exact_manifest(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &TypedItemNode,
    retained: &HirItemKind,
) -> bool {
    let expected = match (attached, retained) {
        (TypedItemNode::Use(attached), HirItemKind::Use(retained)) => {
            use_manifest(parsed, owner, attached, retained).ok()
        }
        _ if matches!(attached, TypedItemNode::Use(_))
            || matches!(retained, HirItemKind::Use(_)) =>
        {
            return false;
        }
        _ => None,
    };
    let is_use_query = |query: &&HirSourceQuery| {
        matches!(
            query,
            HirSourceQuery::Item {
                owner: actual,
                role: HirItemSourceRole::Use(_),
            } if *actual == owner
        )
    };
    match expected {
        Some(expected) => {
            index
                .requirements
                .iter()
                .filter(|(query, _)| is_use_query(query))
                .eq(expected.requirements.iter())
                && index
                    .components
                    .iter()
                    .filter(|(query, _)| is_use_query(query))
                    .eq(expected.components.iter())
        }
        None => {
            index
                .requirements
                .keys()
                .find(|query| is_use_query(query))
                .is_none()
                && index
                    .components
                    .keys()
                    .find(|query| is_use_query(query))
                    .is_none()
        }
    }
}

#[allow(
    clippy::result_large_err,
    reason = "Use manifest rejection preserves complete typed owner and source evidence"
)]
fn use_manifest(
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &AstNode<UseDeclarationKind>,
    retained: &HirUseDeclaration,
) -> Result<UseManifest, HirSourceCommitInvariantError> {
    let tree =
        attached.tree().map_err(
            |error| HirSourceCommitInvariantError::AttachedSyntaxAccess {
                owner: SyntheticOwner::Item(owner),
                error,
            },
        )?;
    if !payload_matches(attached, retained) {
        return Err(
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(owner),
            },
        );
    }
    let mut manifest = UseManifest::default();
    match tree {
        AttachedUseTree::Path { path, alias } => {
            project_binding(
                &mut manifest,
                parsed,
                owner,
                0,
                path.syntax().source_span(),
                path_terminal_source(&path).ok_or_else(|| use_state_mismatch(owner))?,
                alias.as_ref(),
            )?;
        }
        AttachedUseTree::Glob {
            module,
            marker,
            alias,
        } => {
            project_binding(
                &mut manifest,
                parsed,
                owner,
                0,
                module.syntax().source_span(),
                marker,
                alias.as_ref(),
            )?;
        }
        AttachedUseTree::Group {
            module, children, ..
        } => {
            let mut ordinal = 0_u32;
            for child in children {
                let AttachedUseGroupChild::Binding(binding) = child else {
                    continue;
                };
                project_binding(
                    &mut manifest,
                    parsed,
                    owner,
                    ordinal,
                    module.syntax().source_span(),
                    binding.name().source_span(),
                    binding.alias(),
                )?;
                ordinal = ordinal
                    .checked_add(1)
                    .ok_or_else(|| use_state_mismatch(owner))?;
            }
            if usize::try_from(ordinal).ok() != Some(retained.bindings().len()) {
                return Err(use_state_mismatch(owner));
            }
        }
    }
    Ok(manifest)
}

#[allow(
    clippy::result_large_err,
    reason = "Use binding rejection preserves complete typed ordinal and source evidence"
)]
fn project_binding(
    manifest: &mut UseManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    ordinal: u32,
    path: SourceSpan,
    terminal: SourceSpan,
    alias: Option<&AttachedUseAlias>,
) -> Result<(), HirSourceCommitInvariantError> {
    manifest.required(parsed, owner, ordinal, HirUseBindingSourcePart::Path, path)?;
    manifest.required(
        parsed,
        owner,
        ordinal,
        HirUseBindingSourcePart::TerminalReference,
        terminal,
    )?;
    manifest.optional(
        parsed,
        owner,
        ordinal,
        HirUseBindingSourcePart::Alias,
        alias.map(|alias| alias.source_span().clone()),
    )
}

fn path_terminal_source(path: &AttachedPath) -> Option<SourceSpan> {
    path.missing_name()
        .map(arcweft_lang_syntax::attachment::family::FamilyNode::source_span)
        .or_else(|| {
            path.segments()
                .last()
                .map(arcweft_lang_syntax::attachment::source_file::AttachedPathSegment::source_span)
        })
}

fn alias_matches(retained: &HirUseBinding, attached: Option<&AttachedUseAlias>) -> bool {
    match (retained.alias(), attached) {
        (None, None) => true,
        (Some(retained), Some(attached)) if attached.name().kind() != SyntaxKind::MissingName => {
            retained.as_str() == attached.name().source_text()
        }
        (None, Some(attached)) => attached.name().kind() == SyntaxKind::MissingName,
        (Some(_), None | Some(_)) => false,
    }
}

const fn use_binding_query(
    owner: ItemId,
    ordinal: u32,
    part: HirUseBindingSourcePart,
) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Use(HirUseSourceRole::Binding { ordinal, part }),
    }
}

const fn use_state_mismatch(owner: ItemId) -> HirSourceCommitInvariantError {
    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
        owner: SyntheticOwner::Item(owner),
    }
}
