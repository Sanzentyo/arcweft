//! Sole final-HIR source manifest for View-specific item components.

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::{
    AttachedDeclarationPublicId, AttachedViewBody, AttachedViewExport, AttachedViewPartPath,
    TypedItemNode,
};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_source::SourceSpan;

use crate::identity::{ItemId, SyntheticOwner};
use crate::item::{HirItemKind, HirViewDeclaration};
use crate::source_index::{
    HirItemSourceRole, HirSourceCommitInvariantError, HirSourceIndex, HirSourceQuery,
    HirSourceQueryError, HirSourceRequirement, HirSourceSite, HirViewBodySourcePart,
    HirViewExportSourcePart, HirViewSourceRole, StagedHirSourceIndex,
};

#[derive(Default)]
struct ViewManifest {
    requirements: BTreeMap<HirSourceQuery, HirSourceRequirement>,
    components: BTreeMap<HirSourceQuery, HirSourceSite>,
}

impl ViewManifest {
    #[allow(
        clippy::result_large_err,
        reason = "required View rows preserve complete typed query and source evidence"
    )]
    fn required(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        role: HirViewSourceRole,
        span: SourceSpan,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.insert(
            parsed,
            owner,
            role,
            HirSourceRequirement::Required,
            Some(span),
        )
    }

    #[allow(
        clippy::result_large_err,
        reason = "optional View rows preserve complete typed query and source evidence"
    )]
    fn optional(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        role: HirViewSourceRole,
        span: Option<SourceSpan>,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.insert(
            parsed,
            owner,
            role,
            if span.is_some() {
                HirSourceRequirement::Required
            } else {
                HirSourceRequirement::Optional
            },
            span,
        )
    }

    #[allow(
        clippy::result_large_err,
        reason = "View manifest insertion preserves complete typed query and source evidence"
    )]
    fn insert(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        role: HirViewSourceRole,
        requirement: HirSourceRequirement,
        span: Option<SourceSpan>,
    ) -> Result<(), HirSourceCommitInvariantError> {
        let query = view_query(owner, role);
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
    /// Stages the exact View identity/body/export manifest from attached syntax.
    ///
    /// Callable parameter components and body value expressions retain their
    /// existing callable and expression owners. No source text is inspected or
    /// reconstructed here.
    #[allow(
        clippy::result_large_err,
        reason = "View staging preserves complete typed owner and manifest evidence"
    )]
    pub(crate) fn stage_attached_view(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        attached: &TypedItemNode,
        retained: &HirItemKind,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        let manifest = match view_manifest(parsed, owner, attached, retained) {
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

impl HirViewDeclaration {
    pub(crate) fn validate_source_role(
        &self,
        owner: ItemId,
        role: HirViewSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        match role {
            HirViewSourceRole::Whole | HirViewSourceRole::ItemId | HirViewSourceRole::Body(_) => {
                Ok(())
            }
            HirViewSourceRole::Export { ordinal, .. }
                if usize::try_from(ordinal)
                    .ok()
                    .is_some_and(|ordinal| ordinal < self.exports().len()) =>
            {
                Ok(())
            }
            HirViewSourceRole::Export { .. } => Err(HirSourceQueryError::ItemRoleNotApplicable {
                owner,
                role: HirItemSourceRole::View(role),
            }),
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
    let Ok(expected) = view_manifest(parsed, owner, attached, retained) else {
        return false;
    };
    let is_view_query = |candidate: &&HirSourceQuery| {
        matches!(
            candidate,
            HirSourceQuery::Item {
                owner: actual,
                role: HirItemSourceRole::View(_),
            } if *actual == owner
        )
    };
    index
        .requirements
        .iter()
        .filter(|(candidate, _)| is_view_query(candidate))
        .eq(expected.requirements.iter())
        && index
            .components
            .iter()
            .filter(|(candidate, _)| is_view_query(candidate))
            .eq(expected.components.iter())
}

#[allow(
    clippy::result_large_err,
    reason = "View manifest rejection preserves complete typed owner and source evidence"
)]
fn view_manifest(
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &TypedItemNode,
    retained: &HirItemKind,
) -> Result<ViewManifest, HirSourceCommitInvariantError> {
    let (attached, retained) = match (attached, retained) {
        (TypedItemNode::View(attached), HirItemKind::View(retained)) => {
            let attached = attached.semantics().map_err(|error| {
                HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Item(owner),
                    error,
                }
            })?;
            (attached, retained)
        }
        _ if matches!(attached, TypedItemNode::View(_))
            || matches!(retained, HirItemKind::View(_)) =>
        {
            return Err(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Item(owner),
                },
            );
        }
        _ => return Ok(ViewManifest::default()),
    };
    if attached.syntax().snapshot_id() != parsed.snapshot_id() {
        return Err(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
            expected: parsed.snapshot_id().clone(),
            actual: attached.syntax().snapshot_id().clone(),
        });
    }
    if attached.exports().count() != retained.exports().len() {
        return Err(
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(owner),
            },
        );
    }

    let mut manifest = ViewManifest::default();
    manifest.optional(
        parsed,
        owner,
        HirViewSourceRole::ItemId,
        retained_public_id_source(attached.header().public_id()),
    )?;
    stage_body(&mut manifest, parsed, owner, attached.body())?;
    for (position, export) in attached.exports().enumerate() {
        let ordinal = u32::try_from(position).map_err(|_| {
            HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                owner: SyntheticOwner::Item(owner),
            }
        })?;
        if u32::from(export.source_ordinal()) != ordinal {
            return Err(
                HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                    owner: SyntheticOwner::Item(owner),
                },
            );
        }
        stage_export(&mut manifest, parsed, owner, ordinal, export)?;
    }
    Ok(manifest)
}

#[allow(
    clippy::result_large_err,
    reason = "View body rejection preserves complete typed ordinal and source evidence"
)]
fn stage_body(
    manifest: &mut ViewManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    body: &AttachedViewBody,
) -> Result<(), HirSourceCommitInvariantError> {
    manifest.required(
        parsed,
        owner,
        HirViewSourceRole::Body(HirViewBodySourcePart::Whole),
        body.syntax().source_span(),
    )?;
    match body {
        AttachedViewBody::Missing(_) => {
            for part in [
                HirViewBodySourcePart::OpenDelimiter,
                HirViewBodySourcePart::CloseDelimiter,
                HirViewBodySourcePart::Fragment,
            ] {
                manifest.optional(parsed, owner, HirViewSourceRole::Body(part), None)?;
            }
        }
        AttachedViewBody::Braced {
            open,
            close,
            fragment,
            ..
        } => {
            for (part, span) in [
                (HirViewBodySourcePart::OpenDelimiter, open.source_span()),
                (HirViewBodySourcePart::CloseDelimiter, close.source_span()),
                (
                    HirViewBodySourcePart::Fragment,
                    fragment.syntax().source_span(),
                ),
            ] {
                manifest.required(parsed, owner, HirViewSourceRole::Body(part), span)?;
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::result_large_err,
    reason = "View export rejection preserves complete typed ordinal and source evidence"
)]
fn stage_export(
    manifest: &mut ViewManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    ordinal: u32,
    export: &AttachedViewExport,
) -> Result<(), HirSourceCommitInvariantError> {
    for (part, span) in [
        (
            HirViewExportSourcePart::Whole,
            export.syntax().source_span(),
        ),
        (
            HirViewExportSourcePart::PartKeyword,
            export.part().source_span().clone(),
        ),
        (
            HirViewExportSourcePart::LocalPart,
            view_part_source(export.local_part()),
        ),
        (
            HirViewExportSourcePart::AliasKeyword,
            export.alias().source_span().clone(),
        ),
        (
            HirViewExportSourcePart::PublicPart,
            view_part_source(export.public_part()),
        ),
    ] {
        manifest.required(
            parsed,
            owner,
            HirViewSourceRole::Export { ordinal, part },
            span,
        )?;
    }
    Ok(())
}

fn retained_public_id_source(id: &AttachedDeclarationPublicId) -> Option<SourceSpan> {
    match id {
        AttachedDeclarationPublicId::Derived => None,
        AttachedDeclarationPublicId::Explicit { syntax, .. }
        | AttachedDeclarationPublicId::Recovered { syntax, .. } => Some(syntax.source_span()),
    }
}

fn view_part_source(part: &AttachedViewPartPath) -> SourceSpan {
    match part {
        AttachedViewPartPath::Path(path) => path.syntax().source_span(),
        AttachedViewPartPath::Missing(syntax) => syntax.source_span(),
    }
}

const fn view_query(owner: ItemId, role: HirViewSourceRole) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::View(role),
    }
}
