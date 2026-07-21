//! Compiler-owned admission of launch-selected dialogue presentation.

use super::registration::AcceptedLaunchProfileInput;
use crate::view::CompiledViewProduct;
use arcweft_bundle::resource_codec::{ValidatedViewProduct, ValidatedViewProgramResource};
use arcweft_dialogue::{DialoguePresentationProfile, DialogueProfileRevision};
use arcweft_launch::{
    LaunchProfileSelection, ManifestTokenPath, ManifestTokenSlot, accepted::SourceBackedManifest,
};
use arcweft_manifest_model::ProfileId;
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSetRevision, SourceSpan,
};
use arcweft_view::{ViewId, style::ViewStyleSheetId};
use std::sync::Arc;
use thiserror::Error;

/// Launch-selected dialogue presentation admitted against one immutable
/// compiler View/Style product.
#[derive(Clone, Debug)]
pub struct CheckedDialogueProfile {
    profile_id: ProfileId,
    presentation: DialoguePresentationProfile,
    revision: DialogueProfileRevision,
    product: Arc<ValidatedViewProduct>,
    selected_view_source: SourceSpan,
    selected_style_source: Option<SourceSpan>,
}

/// Failure to join one accepted manifest profile with the compiler product.
#[derive(Clone, Debug, Error)]
pub enum DialogueProfileAdmissionError {
    #[error("selected launch profile does not match the accepted manifest revision: {detail}")]
    ResolvedProfileMismatch { detail: String, primary: SourceSpan },
    #[error(
        "the accepted launch profile and compiler transaction use different resource registries"
    )]
    ResourceRegistryMismatch { primary: SourceSpan },
    #[error("the accepted View product has no View program")]
    MissingViewProgram { view: ViewId, primary: SourceSpan },
    #[error("dialogue View `{view}` is absent from the accepted View program")]
    MissingView { view: ViewId, primary: SourceSpan },
    #[error("View `{view}` does not accept the canonical dialogue parameter role")]
    ViewIsNotDialogue {
        view: ViewId,
        primary: SourceSpan,
        definition: SourceSpan,
    },
    #[error("dialogue Style sheet `{style}` is absent from the accepted Style program")]
    MissingStyle {
        style: ViewStyleSheetId,
        primary: SourceSpan,
    },
    #[error("dialogue profile provenance is incomplete for `{owner}`")]
    MissingSourceProvenance { owner: String, primary: SourceSpan },
    #[error("dialogue profile revision does not match the accepted compiler product: {detail}")]
    RevisionMismatch { detail: String, primary: SourceSpan },
}

struct AdmittedDialogueProduct<'a> {
    product: &'a Arc<ValidatedViewProduct>,
    program: &'a ValidatedViewProgramResource,
    complete_sources: SourceSetRevision,
    view_source: SourceSpan,
    style_source: Option<SourceSpan>,
}

impl CheckedDialogueProfile {
    /// Admits one already-resolved profile against the exact compiler-owned
    /// View/Style product. This method performs no I/O and does not read source
    /// text or a second manifest representation.
    pub fn try_admit(
        input: &AcceptedLaunchProfileInput,
        views: &CompiledViewProduct,
        compiler_resource_types: &Arc<ResourceTypeRegistry>,
    ) -> Result<Self, DialogueProfileAdmissionError> {
        let (presentation, profile_primary) =
            admit_presentation(input, views, compiler_resource_types)?;
        let admitted = admit_product(input, views, &presentation, &profile_primary)?;
        let revision = DialogueProfileRevision::from_admitted_parts(
            input.manifest().document().identity().clone(),
            input.topology_source_revision(),
            admitted.complete_sources,
            admitted.program.program_id().clone(),
            admitted.program.accepted_revision(),
            views.resource_type_registry_digest(),
        );
        Ok(Self {
            profile_id: input.profile_id().clone(),
            presentation,
            revision,
            product: Arc::clone(admitted.product),
            selected_view_source: admitted.view_source,
            selected_style_source: admitted.style_source,
        })
    }

    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub const fn presentation(&self) -> &DialoguePresentationProfile {
        &self.presentation
    }

    pub const fn revision(&self) -> &DialogueProfileRevision {
        &self.revision
    }

    /// Returns the exact accepted product shared with the compiled project.
    pub const fn product(&self) -> &Arc<ValidatedViewProduct> {
        &self.product
    }

    pub const fn selected_view_source(&self) -> &SourceSpan {
        &self.selected_view_source
    }

    pub const fn selected_style_source(&self) -> Option<&SourceSpan> {
        self.selected_style_source.as_ref()
    }
}

fn admit_presentation(
    input: &AcceptedLaunchProfileInput,
    views: &CompiledViewProduct,
    compiler_resource_types: &Arc<ResourceTypeRegistry>,
) -> Result<(DialoguePresentationProfile, SourceSpan), DialogueProfileAdmissionError> {
    let primary = profile_primary(input.manifest(), input.profile_id());
    let resolved = input
        .manifest()
        .resolve_profile(LaunchProfileSelection::Explicit(
            input.profile_id().as_str(),
        ))
        .map_err(
            |report| DialogueProfileAdmissionError::ResolvedProfileMismatch {
                detail: report.to_string(),
                primary: primary.clone(),
            },
        )?;
    if &resolved != input.resolved_profile() || resolved.id() != input.profile_id() {
        return Err(DialogueProfileAdmissionError::ResolvedProfileMismatch {
            detail: "the retained resolved value differs from a pure resolution of the accepted manifest"
                .to_owned(),
            primary,
        });
    }
    if !Arc::ptr_eq(input.resource_types(), compiler_resource_types)
        || input.resource_types().digest() != views.resource_type_registry_digest()
    {
        return Err(DialogueProfileAdmissionError::ResourceRegistryMismatch { primary });
    }
    Ok((resolved.dialogue().clone(), primary))
}

fn admit_product<'a>(
    input: &AcceptedLaunchProfileInput,
    views: &'a CompiledViewProduct,
    presentation: &DialoguePresentationProfile,
    profile_primary: &SourceSpan,
) -> Result<AdmittedDialogueProduct<'a>, DialogueProfileAdmissionError> {
    let view_primary = view_primary(input.manifest(), input.profile_id());
    let product = views.product();
    let program =
        product
            .program()
            .ok_or_else(|| DialogueProfileAdmissionError::MissingViewProgram {
                view: presentation.view().clone(),
                primary: view_primary.clone(),
            })?;
    let complete_sources = views.product_source_revision();
    if program.source_set_revision() != complete_sources {
        return Err(DialogueProfileAdmissionError::RevisionMismatch {
            detail: "View program and complete product source revisions differ".to_owned(),
            primary: profile_primary.clone(),
        });
    }
    if let Some(style) = product.style()
        && style.source_set_revision() != complete_sources
    {
        return Err(DialogueProfileAdmissionError::RevisionMismatch {
            detail: "Style program and complete product source revisions differ".to_owned(),
            primary: profile_primary.clone(),
        });
    }

    let definition = program.definition(presentation.view()).ok_or_else(|| {
        DialogueProfileAdmissionError::MissingView {
            view: presentation.view().clone(),
            primary: view_primary.clone(),
        }
    })?;
    let view_source = views
        .view_source(presentation.view())
        .cloned()
        .ok_or_else(|| DialogueProfileAdmissionError::MissingSourceProvenance {
            owner: presentation.view().as_str().to_owned(),
            primary: view_primary.clone(),
        })?;
    if !definition.accepts_dialogue_input() {
        return Err(DialogueProfileAdmissionError::ViewIsNotDialogue {
            view: presentation.view().clone(),
            primary: view_primary,
            definition: view_source,
        });
    }
    let style_source = admit_style(input, views, product, presentation.style())?;
    Ok(AdmittedDialogueProduct {
        product,
        program,
        complete_sources,
        view_source,
        style_source,
    })
}

fn admit_style(
    input: &AcceptedLaunchProfileInput,
    views: &CompiledViewProduct,
    product: &ValidatedViewProduct,
    style: Option<&ViewStyleSheetId>,
) -> Result<Option<SourceSpan>, DialogueProfileAdmissionError> {
    style
        .map(|style_id| {
            let primary = style_primary(input.manifest(), input.profile_id());
            if product
                .style()
                .and_then(|style| style.program().sheet(style_id))
                .is_none()
            {
                return Err(DialogueProfileAdmissionError::MissingStyle {
                    style: style_id.clone(),
                    primary,
                });
            }
            views.style_source(style_id).cloned().ok_or_else(|| {
                DialogueProfileAdmissionError::MissingSourceProvenance {
                    owner: style_id.as_str().to_owned(),
                    primary,
                }
            })
        })
        .transpose()
}

impl DialogueProfileAdmissionError {
    pub(crate) fn diagnostic(&self) -> Diagnostic {
        let (code, primary) = match self {
            Self::MissingViewProgram { primary, .. } | Self::MissingView { primary, .. } => {
                ("profile.dialogue.view.missing", primary)
            }
            Self::ViewIsNotDialogue { primary, .. } => {
                ("profile.dialogue.view.not-dialogue", primary)
            }
            Self::MissingStyle { primary, .. } => ("profile.dialogue.style.missing", primary),
            Self::ResolvedProfileMismatch { primary, .. }
            | Self::ResourceRegistryMismatch { primary }
            | Self::MissingSourceProvenance { primary, .. }
            | Self::RevisionMismatch { primary, .. } => {
                ("profile.dialogue.revision.mismatch", primary)
            }
        };
        let mut diagnostic = Diagnostic::new(DiagnosticSeverity::Error, self.to_string())
            .with_code(code)
            .with_label(DiagnosticLabel::primary(
                primary.clone(),
                Some("this launch profile could not be admitted".to_owned()),
            ));
        if let Self::ViewIsNotDialogue { definition, .. } = self {
            diagnostic = diagnostic.with_label(DiagnosticLabel::secondary(
                definition.clone(),
                Some("the selected View is defined here".to_owned()),
            ));
        }
        diagnostic
    }
}

fn profile_primary(manifest: &SourceBackedManifest, profile: &ProfileId) -> SourceSpan {
    manifest
        .manifest_token_span(
            &ManifestTokenPath::ProfileDialogueTable {
                profile: profile.clone(),
            },
            ManifestTokenSlot::TableHeader,
        )
        .or_else(|| {
            manifest.manifest_token_span(
                &ManifestTokenPath::ProfileTable {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::TableHeader,
            )
        })
        .cloned()
        .unwrap_or_else(|| manifest.document().start_span())
}

fn view_primary(manifest: &SourceBackedManifest, profile: &ProfileId) -> SourceSpan {
    manifest
        .manifest_token_span(
            &ManifestTokenPath::ProfileDialogueView {
                profile: profile.clone(),
            },
            ManifestTokenSlot::Value,
        )
        .cloned()
        .unwrap_or_else(|| profile_primary(manifest, profile))
}

fn style_primary(manifest: &SourceBackedManifest, profile: &ProfileId) -> SourceSpan {
    manifest
        .manifest_token_span(
            &ManifestTokenPath::ProfileDialogueStyle {
                profile: profile.clone(),
            },
            ManifestTokenSlot::Value,
        )
        .cloned()
        .unwrap_or_else(|| profile_primary(manifest, profile))
}
