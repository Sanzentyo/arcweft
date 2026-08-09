//! Compiler-owned admission boundary for final-HIR native Style data.
//!
//! The former syntax/flattened-HIR Style reader has been removed. Until the
//! final-HIR semantic producer is connected, authored Style items fail closed;
//! an empty project receives the one canonical empty product artifact.

use std::collections::BTreeMap;

use arcweft_bundle::resource_codec::{SourceRangeRef, view::ViewStyleResource};
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_lang_hir::{
    identity::ItemId,
    item::{
        HirItemKind, HirStyleAssignOperation, HirStyleBodyItem, HirStyleCombinator,
        HirStyleSelector,
    },
    module::HirModule,
    project::HirProject,
    source_index::{
        HirItemSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStyleBodyPath,
        HirStyleBodySourcePart, HirStyleSourceRole,
    },
};
use arcweft_lang_sema::final_analysis::{CheckedExpressionResolution, FinalSemanticAnalysis};
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_source::{ProductSourceRef, SourceSpan};
use arcweft_view::{
    ViewElementKind,
    style::{
        ViewPropertyKind, ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleCombinator,
        ViewStyleDeclaration, ViewStyleProgram, ViewStyleRule, ViewStyleSelectorSequence,
        ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
    },
};
use thiserror::Error;

const VIEW_STYLE_PROGRAM_ID: &str = "view.style.program";

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
        .executable_view()
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
        let reference = style
            .id()
            .as_resolved()
            .ok_or(ViewStyleLowerError::InvalidStyleIdentity { owner })?;
        let public_id = reference
            .declaration_public_id(DeclarationIdentityFamily::Style)
            .ok_or(ViewStyleLowerError::InvalidStyleIdentity { owner })?;
        let sheet_id = ViewStyleSheetId::try_new(public_id.as_str().to_owned())
            .map_err(|_| ViewStyleLowerError::InvalidStyleIdentity { owner })?;
        let sheet_source = style_source_span(module, owner, HirStyleSourceRole::ItemId, "item ID")?;
        if sources.insert(sheet_id.clone(), sheet_source).is_some() {
            return Err(ViewStyleLowerError::InvalidStyleIdentity { owner });
        }
        if !style.tokens().is_empty() {
            return Err(ViewStyleLowerError::MissingCheckedStyleProjection { owner });
        }
        let mut source_order = 0_u32;
        let rules = lower_style_body(
            module,
            owner,
            style.body(),
            analysis,
            &mut product_sources,
            &mut source_ranges,
            &mut source_order,
        )?;
        sheets.push(ViewStyleSheet::new(sheet_id, Vec::new(), rules)?);
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

#[allow(
    clippy::too_many_arguments,
    reason = "Style body lowering owns recursive source order and the one resource source table"
)]
fn lower_style_body(
    module: &HirModule,
    owner: ItemId,
    body: &[HirStyleBodyItem],
    analysis: &FinalSemanticAnalysis,
    product_sources: &mut Vec<ProductSourceRef>,
    source_ranges: &mut Vec<SourceRangeRef>,
    source_order: &mut u32,
) -> Result<Vec<ViewStyleRule>, ViewStyleLowerError> {
    let mut rules = Vec::new();
    for (rule_ordinal, item) in body.iter().enumerate() {
        let HirStyleBodyItem::Rule(rule) = item else {
            return Err(ViewStyleLowerError::MissingCheckedStyleProjection { owner });
        };
        let selector = lower_selector(rule.selector())
            .ok_or(ViewStyleLowerError::MissingCheckedStyleProjection { owner })?;
        let path = HirStyleBodyPath::root();
        let rule_source = style_product_source(
            module,
            owner,
            HirStyleSourceRole::Body {
                path: path.clone(),
                part: HirStyleBodySourcePart::RuleSelector {
                    rule: u32::try_from(rule_ordinal).map_err(|_| {
                        ViewStyleLowerError::MissingCheckedStyleProjection { owner }
                    })?,
                },
            },
            "rule selector",
            product_sources,
            source_ranges,
        )?;
        let mut declarations = Vec::new();
        for (declaration_ordinal, declaration) in rule.declarations().iter().enumerate() {
            let property = declaration
                .property()
                .as_str()
                .and_then(ViewPropertyKind::from_source_name)
                .ok_or(ViewStyleLowerError::MissingCheckedStyleProjection { owner })?;
            let checked = analysis
                .expression(declaration.value())
                .ok_or(ViewStyleLowerError::MissingCheckedStyleProjection { owner })?;
            let CheckedExpressionResolution::StyleValue(value) = checked.resolution() else {
                return Err(ViewStyleLowerError::MissingCheckedStyleProjection { owner });
            };
            let declaration_source = style_product_source(
                module,
                owner,
                HirStyleSourceRole::Body {
                    path: path.clone(),
                    part: HirStyleBodySourcePart::DeclarationWhole {
                        rule: u32::try_from(rule_ordinal).map_err(|_| {
                            ViewStyleLowerError::MissingCheckedStyleProjection { owner }
                        })?,
                        declaration: u32::try_from(declaration_ordinal).map_err(|_| {
                            ViewStyleLowerError::MissingCheckedStyleProjection { owner }
                        })?,
                    },
                },
                "declaration",
                product_sources,
                source_ranges,
            )?;
            let op = match declaration.operation() {
                HirStyleAssignOperation::Replace => ViewStyleAssignOp::Replace,
                HirStyleAssignOperation::Append => ViewStyleAssignOp::Append,
                HirStyleAssignOperation::Recovered(_) => {
                    return Err(ViewStyleLowerError::MissingCheckedStyleProjection { owner });
                }
            };
            declarations.push(ViewStyleDeclaration::new(
                property,
                value.clone(),
                op,
                declaration_source,
            )?);
        }
        rules.push(ViewStyleRule::new(
            selector,
            None,
            declarations,
            *source_order,
            rule_source,
        )?);
        *source_order = source_order
            .checked_add(1)
            .ok_or(ViewStyleLowerError::MissingCheckedStyleProjection { owner })?;
    }
    Ok(rules)
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
            let element = match sequence.element() {
                Some(name) => Some(ViewElementKind::from_source_name(name.as_str()?)?),
                None => None,
            };
            if sequence.element().is_some() && element.is_none() {
                return None;
            }
            if sequence.part().is_some() || !sequence.predicates().is_empty() {
                return None;
            }
            ViewStyleSelectorSequence::new(relation, element, None, Vec::new())
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
