//! Compiler-owned admission boundary for final-HIR native Style data.
//!
//! Authored selectors consume typed final-HIR components and declarations
//! consume their accepted semantic values. Unsupported products fail closed.

use std::collections::BTreeMap;

use arcweft_bundle::resource_codec::{SourceRangeRef, view::ViewStyleResource};
use arcweft_id::PublicId;
use arcweft_lang_hir::{
    identity::ItemId,
    item::{HirItemKind, HirStyleCombinator, HirStyleSelector},
    module::HirModule,
    project::HirProject,
    source_index::{
        HirItemSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStyleSourceRole,
        HirStyleTokenSourcePart,
    },
};
use arcweft_lang_sema::final_analysis::{CheckedExpressionResolution, FinalSemanticAnalysis};
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_source::{ProductSourceRef, SourceSpan};
use arcweft_view::style::{
    ViewStyleApplicationTarget, ViewStyleCombinator, ViewStyleProgram, ViewStyleSelectorSequence,
    ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId, ViewStyleTargetKind, ViewStyleToken,
    ViewStyleTokenId,
};
use thiserror::Error;

const VIEW_STYLE_PROGRAM_ID: &str = "view.style.program";

#[path = "style/body.rs"]
mod body;

/// Complete compiler-owned Style output for one accepted project generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledViewStyleArtifact {
    resource: ViewStyleResource,
    applications: ViewStyleApplicationLookup,
    sources: BTreeMap<ViewStyleSheetId, SourceSpan>,
}

/// Typed Style applications indexed inside their owning final-HIR View.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewStyleApplicationLookup {
    views: BTreeMap<PublicId, ViewStyleViewApplications>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ViewStyleViewApplications {
    root: Vec<ViewStyleApplicationTarget>,
    sites: Vec<ViewStyleApplicationSite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ViewStyleApplicationSite {
    range: TextRange,
    applications: Vec<ViewStyleApplicationTarget>,
}

/// Checked Style lowering failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewStyleLowerError {
    #[error("final-HIR Style item {owner:?} has no checked Style projection")]
    MissingCheckedStyleProjection { owner: ItemId },
    #[error("final-HIR Style item {owner:?} has no valid declaration identity")]
    InvalidStyleIdentity { owner: ItemId },
    #[error("final-HIR Style item {owner:?} is missing source role {role}")]
    MissingStyleSource { owner: ItemId, role: &'static str },
    #[error(transparent)]
    Model(#[from] arcweft_view::style::ViewStyleModelError),
    #[error(transparent)]
    Environment(#[from] arcweft_view::style::ViewEnvironmentConditionError),
    #[error(transparent)]
    Product(#[from] arcweft_bundle::resource_codec::ViewProductBuildError),
    #[error(transparent)]
    ProductSource(#[from] arcweft_source::ProductSourceIdentityError),
}

impl CompiledViewStyleArtifact {
    pub const fn resource(&self) -> &ViewStyleResource {
        &self.resource
    }

    pub const fn applications(&self) -> &ViewStyleApplicationLookup {
        &self.applications
    }

    pub const fn sources(&self) -> &BTreeMap<ViewStyleSheetId, SourceSpan> {
        &self.sources
    }

    pub fn into_parts(
        self,
    ) -> (
        ViewStyleResource,
        ViewStyleApplicationLookup,
        BTreeMap<ViewStyleSheetId, SourceSpan>,
    ) {
        (self.resource, self.applications, self.sources)
    }
}

impl ViewStyleApplicationLookup {
    pub fn root_applications_for(&self, view: &PublicId) -> &[ViewStyleApplicationTarget] {
        self.views
            .get(view)
            .map_or(&[], |applications| applications.root.as_slice())
    }

    pub fn applications_for(
        &self,
        view: &PublicId,
        node_range: TextRange,
    ) -> &[ViewStyleApplicationTarget] {
        self.views
            .get(view)
            .and_then(|applications| {
                applications
                    .sites
                    .iter()
                    .find(|site| site.range == node_range)
            })
            .map_or(&[], |site| site.applications.as_slice())
    }

    pub fn view_ids(&self) -> impl Iterator<Item = &PublicId> {
        self.views.keys()
    }
}

/// Publishes Style data from the accepted final semantic generation.
///
/// # Panics
///
/// Panics if the compiler calls this boundary before the HIR project has been
/// admitted as executable.
pub fn lower_project_view_styles(
    hir_project: &HirProject,
    analysis: &FinalSemanticAnalysis,
) -> Result<CompiledViewStyleArtifact, ViewStyleLowerError> {
    let executable = hir_project
        .analysis_view()
        .expect("project compilation admits only executable final-HIR modules before Style");
    let mut sheets = Vec::new();
    let mut sources = BTreeMap::new();
    let mut product_sources = Vec::new();
    let mut source_ranges = Vec::new();
    for item in executable.items() {
        let HirItemKind::Style(style) = item.item().kind() else {
            continue;
        };
        let module = item.module();
        let owner = item.id();
        let public_id = style
            .public_id()
            .resolved()
            .ok_or(ViewStyleLowerError::InvalidStyleIdentity { owner })?;
        let sheet_id = ViewStyleSheetId::try_new(public_id.as_str().to_owned())
            .map_err(|_| ViewStyleLowerError::InvalidStyleIdentity { owner })?;
        let sheet_source = style_source_span(module, owner, HirStyleSourceRole::ItemId, "item ID")?;
        if sources.insert(sheet_id.clone(), sheet_source).is_some() {
            return Err(ViewStyleLowerError::InvalidStyleIdentity { owner });
        }
        let mut tokens = Vec::with_capacity(style.tokens().len());
        for (ordinal, token) in style.tokens().iter().enumerate() {
            let id = PublicId::try_from(token)
                .map_err(|_| ViewStyleLowerError::InvalidStyleIdentity { owner })?;
            let checked = analysis
                .expression(token.value())
                .ok_or(ViewStyleLowerError::MissingCheckedStyleProjection { owner })?;
            let CheckedExpressionResolution::StyleValue(value) = checked.resolution() else {
                return Err(ViewStyleLowerError::MissingCheckedStyleProjection { owner });
            };
            if checked.value_type()
                != Some(&arcweft_lang_sema::types::TypeKind::CompileTimeStyle(
                    arcweft_lang_sema::types::CompileTimeStyleType::Value(value.kind()),
                ))
            {
                return Err(ViewStyleLowerError::MissingCheckedStyleProjection { owner });
            }
            let source = style_product_source(
                module,
                owner,
                HirStyleSourceRole::Token {
                    ordinal: u32::try_from(ordinal).map_err(|_| {
                        ViewStyleLowerError::MissingCheckedStyleProjection { owner }
                    })?,
                    part: HirStyleTokenSourcePart::Whole,
                },
                "token declaration",
                &mut product_sources,
                &mut source_ranges,
            )?;
            tokens.push(ViewStyleToken::new(
                ViewStyleTokenId::from_public_id(id),
                value.kind(),
                value.clone(),
                source,
            )?);
        }
        let rules = body::StyleBodyLowerer::new(
            module,
            owner,
            analysis,
            &mut product_sources,
            &mut source_ranges,
        )
        .lower(style.body())?;
        sheets.push(ViewStyleSheet::new(sheet_id, tokens, rules)?);
    }

    let program = ViewStyleProgram::try_new(sheets, Vec::new())?;

    Ok(CompiledViewStyleArtifact {
        resource: ViewStyleResource {
            style_program_id: VIEW_STYLE_PROGRAM_ID.to_owned(),
            program,
            source_refs: product_sources,
            source_map_refs: source_ranges,
            ..ViewStyleResource::default()
        },
        applications: ViewStyleApplicationLookup::default(),
        sources,
    })
}

fn lower_selector(selector: &HirStyleSelector) -> Option<arcweft_view::style::ViewStyleSelector> {
    let HirStyleSelector::Resolved(sequences) = selector else {
        return None;
    };
    let sequences = sequences
        .iter()
        .map(|sequence| {
            let relation = sequence
                .relation_to_previous()
                .map(|relation| match relation {
                    HirStyleCombinator::Descendant => ViewStyleCombinator::Descendant,
                    HirStyleCombinator::Child => ViewStyleCombinator::Child,
                });
            let target = match sequence.element() {
                Some(name) => Some(ViewStyleTargetKind::from_source_name(name.as_str()?)?),
                None => None,
            };
            let part = match sequence.part() {
                Some(name) => Some(arcweft_view::ViewPartName::try_new(name.as_str()?).ok()?),
                None => None,
            };
            let predicates = sequence
                .predicates()
                .iter()
                .map(|name| {
                    let name = name.as_str()?;
                    arcweft_view::style::ViewInteractionSelector::from_source_name(name)
                        .map(arcweft_view::style::ViewStylePredicate::Interaction)
                        .or_else(|| {
                            arcweft_view::style::ViewElementState::from_source_name(name)
                                .map(arcweft_view::style::ViewStylePredicate::ElementState)
                        })
                })
                .collect::<Option<Vec<_>>>()?;
            ViewStyleSelectorSequence::new(relation, target, part, predicates)
        })
        .collect::<Option<Vec<_>>>()?;
    arcweft_view::style::ViewStyleSelector::new(sequences)
}

fn style_product_source(
    module: &HirModule,
    owner: ItemId,
    role: HirStyleSourceRole,
    label: &'static str,
    product_sources: &mut Vec<ProductSourceRef>,
    source_ranges: &mut Vec<SourceRangeRef>,
) -> Result<ViewStyleSourceId, ViewStyleLowerError> {
    let span = style_source_span(module, owner, role, label)?;
    let source = ProductSourceRef::try_for_identity(span.source())?;
    if !product_sources.iter().any(|candidate| candidate == &source) {
        product_sources.push(source.clone());
    }
    let start = u32::try_from(span.range().start())
        .map_err(|_| ViewStyleLowerError::MissingCheckedStyleProjection { owner })?;
    let end = u32::try_from(span.range().end())
        .map_err(|_| ViewStyleLowerError::MissingCheckedStyleProjection { owner })?;
    source_ranges.push(SourceRangeRef::try_for_source(
        product_sources,
        &source,
        start,
        end,
    )?);
    let ordinal = u32::try_from(source_ranges.len() - 1)
        .map_err(|_| ViewStyleLowerError::MissingCheckedStyleProjection { owner })?;
    Ok(ViewStyleSourceId::new(ordinal))
}

fn style_source_span(
    module: &HirModule,
    owner: ItemId,
    role: HirStyleSourceRole,
    label: &'static str,
) -> Result<SourceSpan, ViewStyleLowerError> {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Item {
                owner,
                role: HirItemSourceRole::Style(role),
            },
        )
        .map_err(|_| ViewStyleLowerError::MissingStyleSource { owner, role: label })?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => {
            Err(ViewStyleLowerError::MissingStyleSource { owner, role: label })
        }
    }
}
