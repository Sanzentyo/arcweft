//! Compiler-owned lowering of retained image declarations.

use std::collections::BTreeMap;

use arcweft_bundle::{
    BundleImageObject, BundleImageObjectAlignment, BundleImageObjectBounds, BundleImageObjectFit,
    BundleImageObjectParam, BundleImageObjectPlayback, BundleImageObjectProxy,
    BundleImageObjectTransform,
};
use arcweft_id::{DeclarationIdentityFamily, IdError, PublicId};
use arcweft_lang_hir::{model::HirTopLevelDecl, project::HirProject};
use arcweft_lang_syntax::{
    ast::items::{EntityDeclItem, EntityDeclKind, ImageDeclBody, ImageDeclField},
    expr::{CallArg, Expr, Literal, UnaryOp},
    literal::{DurationUnit, UnitNumberSuffix},
};
use arcweft_layout::{
    LayoutSize,
    stage_placement::{
        StageAnchor, StageInsets, StagePlacement, StagePlacementContext, StageRect,
        StageScalePolicy, StageSize,
    },
};
use arcweft_presentation::image::ImageObjectId;
use arcweft_project::sources::ProjectSources;
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceDocument, SourceDocumentIdentity,
    SourceRange, SourceSpan, SourceSpanError,
};
use num_traits::ToPrimitive;
use thiserror::Error;

/// Retained image objects accepted as part of one compiler transaction.
#[derive(Clone, Debug, Default)]
pub struct CompiledImageCatalog {
    objects: Vec<BundleImageObject>,
    sources: BTreeMap<ImageObjectId, SourceSpan>,
}

/// Failure to lower a typed image declaration.
#[derive(Debug, Error)]
pub enum ImageCompileError {
    #[error("HIR project module `{module}` has no matching project source document")]
    MissingProjectSource { module: String },
    #[error("project source module `{module}` has no matching lowered HIR module")]
    MissingHirProjectModule { module: String },
    #[error("project source module `{module}` does not match its HIR source identity")]
    ProjectHirSourceMismatch {
        module: String,
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
    #[error("image `{image}` has an invalid source range: {source}")]
    InvalidSourceRange {
        image: String,
        source: SourceSpanError,
    },
    #[error("image `{image}` has an invalid nominal identity: {source}")]
    InvalidIdentity {
        image: String,
        source: IdError,
        span: SourceSpan,
    },
    #[error("image `{image}` is declared more than once")]
    DuplicateImage {
        image: String,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    #[error("image `{image}` has no structured declaration body")]
    MissingBody { image: String, span: SourceSpan },
    #[error("image `{image}` repeats field `{field}`")]
    DuplicateField {
        image: String,
        field: String,
        span: SourceSpan,
    },
    #[error("image `{image}` is missing required field `{field}`")]
    MissingField {
        image: String,
        field: &'static str,
        span: SourceSpan,
    },
    #[error("image `{image}` has invalid `{field}`: {reason}")]
    InvalidField {
        image: String,
        field: String,
        reason: &'static str,
        span: SourceSpan,
    },
    #[error("image `{image}` uses unsupported field `{field}`")]
    UnsupportedField {
        image: String,
        field: String,
        span: SourceSpan,
    },
    #[error("image `{image}` stage placement failed: {reason}")]
    Placement {
        image: String,
        reason: String,
        span: SourceSpan,
    },
}

impl CompiledImageCatalog {
    /// Accepted image objects in deterministic public-ID order.
    pub fn objects(&self) -> &[BundleImageObject] {
        &self.objects
    }

    /// Exact declaration owner for an accepted image object.
    pub fn source(&self, id: &ImageObjectId) -> Option<&SourceSpan> {
        self.sources.get(id)
    }

    /// Asset IDs referenced by retained image declarations.
    pub fn asset_refs(&self) -> impl Iterator<Item = &str> {
        self.objects.iter().map(|object| object.asset.as_str())
    }
}

impl ImageCompileError {
    /// Structured project diagnostic retaining the declaration owner.
    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::MissingProjectSource { module } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("HIR project module `{module}` has no matching project source document"),
            )
            .with_code("compiler.image.missing_project_source"),
            Self::MissingHirProjectModule { module } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("project source module `{module}` has no matching lowered HIR module"),
            )
            .with_code("compiler.image.missing_hir_project_module"),
            Self::ProjectHirSourceMismatch { module, .. } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("project source module `{module}` does not match its HIR source identity"),
            )
            .with_code("compiler.image.project_hir_source_mismatch"),
            Self::InvalidSourceRange { image, source } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("image `{image}` has an invalid source range: {source}"),
            )
            .with_code("compiler.image.invalid_source_range"),
            Self::InvalidIdentity {
                image,
                source,
                span,
            } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("image `{image}` has an invalid nominal identity: {source}"),
            )
            .with_code("compiler.image.invalid_identity")
            .with_span(span.clone()),
            Self::DuplicateImage {
                image,
                first,
                duplicate,
            } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("image `{image}` is declared more than once"),
            )
            .with_code("compiler.image.duplicate")
            .with_label(DiagnosticLabel::primary(
                duplicate.clone(),
                Some("duplicate image declaration".to_owned()),
            ))
            .with_label(DiagnosticLabel::secondary(
                first.clone(),
                Some("first image declaration".to_owned()),
            )),
            Self::MissingBody { image, span } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("image `{image}` has no structured declaration body"),
            )
            .with_code("compiler.image.missing_body")
            .with_span(span.clone()),
            Self::DuplicateField { image, field, span } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("image `{image}` repeats field `{field}`"),
            )
            .with_code("compiler.image.duplicate_field")
            .with_span(span.clone()),
            Self::MissingField { image, field, span } => {
                Self::missing_field_diagnostic(image, field, span)
            }
            Self::InvalidField {
                image,
                field,
                reason,
                span,
            } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("image `{image}` has invalid `{field}`: {reason}"),
            )
            .with_code("compiler.image.invalid_field")
            .with_span(span.clone()),
            Self::UnsupportedField { image, field, span } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("image `{image}` uses unsupported field `{field}`"),
            )
            .with_code("compiler.image.unsupported_field")
            .with_span(span.clone()),
            Self::Placement {
                image,
                reason,
                span,
            } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("image `{image}` stage placement failed: {reason}"),
            )
            .with_code("compiler.image.stage_placement")
            .with_span(span.clone()),
        }
    }

    fn missing_field_diagnostic(image: &str, field: &'static str, span: &SourceSpan) -> Diagnostic {
        Diagnostic::new(
            DiagnosticSeverity::Error,
            format!("image object `{image}` is missing required field `{field}`"),
        )
        .with_code(if field == "asset" {
            "bundle.image.missing_asset_reference"
        } else {
            "compiler.image.missing_field"
        })
        .with_label(DiagnosticLabel::primary(
            span.clone(),
            Some(format!("this image declaration requires `{field}`")),
        ))
    }
}

/// Lowers every typed image declaration from the exact module-preserving HIR.
pub fn lower_project_images(
    hir_project: &HirProject,
    project: &ProjectSources,
) -> Result<CompiledImageCatalog, ImageCompileError> {
    for source in project.modules() {
        let module = source.module();
        let expected = hir_project.source(module).ok_or_else(|| {
            ImageCompileError::MissingHirProjectModule {
                module: module.to_string(),
            }
        })?;
        if expected != source.document().identity() {
            return Err(ImageCompileError::ProjectHirSourceMismatch {
                module: module.to_string(),
                expected: Box::new(expected.clone()),
                actual: Box::new(source.document().identity().clone()),
            });
        }
    }
    let mut objects = BTreeMap::<String, BundleImageObject>::new();
    let mut sources = BTreeMap::<ImageObjectId, SourceSpan>::new();
    for (module, hir) in hir_project.modules() {
        let source =
            project
                .module(module)
                .ok_or_else(|| ImageCompileError::MissingProjectSource {
                    module: module.to_string(),
                })?;
        for declaration in hir.declarations().iter().filter_map(|declaration| {
            let HirTopLevelDecl::EntityDecl(item) = declaration else {
                return None;
            };
            (item.kind() == EntityDeclKind::Image).then_some(item)
        }) {
            let id = declaration.id().body().to_owned();
            let range = declaration.range();
            let span = source
                .document()
                .span(SourceRange::new(range.start(), range.end()))
                .map_err(|source| ImageCompileError::InvalidSourceRange {
                    image: id.clone(),
                    source,
                })?;
            let typed_id = ImageObjectId::new(PublicId::try_new(id.clone()).map_err(|source| {
                ImageCompileError::InvalidIdentity {
                    image: id.clone(),
                    source,
                    span: span.clone(),
                }
            })?);
            if let Some(first) = sources.insert(typed_id, span.clone()) {
                return Err(ImageCompileError::DuplicateImage {
                    image: id,
                    first,
                    duplicate: span,
                });
            }
            let body = declaration
                .image_body()
                .ok_or_else(|| ImageCompileError::MissingBody {
                    image: id.clone(),
                    span: span.clone(),
                })?;
            objects.insert(
                id.clone(),
                lower_image(declaration, body, span, source.document())?,
            );
        }
    }
    Ok(CompiledImageCatalog {
        objects: objects.into_values().collect(),
        sources,
    })
}

struct ImageFields<'a> {
    image: &'a str,
    source: SourceSpan,
    document: &'a SourceDocument,
    fields: BTreeMap<&'a str, &'a ImageDeclField>,
}

impl<'a> ImageFields<'a> {
    fn try_new(
        image: &'a str,
        source: SourceSpan,
        document: &'a SourceDocument,
        body: &'a ImageDeclBody,
    ) -> Result<Self, ImageCompileError> {
        let mut fields = BTreeMap::new();
        for field in body.fields() {
            if fields.insert(field.name(), field).is_some() {
                return Err(ImageCompileError::DuplicateField {
                    image: image.to_owned(),
                    field: field.name().to_owned(),
                    span: document
                        .span(SourceRange::new(field.whole().start(), field.whole().end()))
                        .map_err(|source| ImageCompileError::InvalidSourceRange {
                            image: image.to_owned(),
                            source,
                        })?,
                });
            }
        }
        Ok(Self {
            image,
            source,
            document,
            fields,
        })
    }

    fn get(&self, name: &str) -> Option<&'a Expr> {
        self.fields.get(name).map(|field| field.value())
    }

    fn field(&self, name: &str) -> Option<&'a ImageDeclField> {
        self.fields.get(name).copied()
    }

    fn required(&self, name: &'static str) -> Result<&'a Expr, ImageCompileError> {
        self.get(name)
            .ok_or_else(|| ImageCompileError::MissingField {
                image: self.image.to_owned(),
                field: name,
                span: self.source.clone(),
            })
    }

    fn invalid(&self, field: impl Into<String>, reason: &'static str) -> ImageCompileError {
        let field = field.into();
        let span = self
            .field(&field)
            .and_then(|field| {
                self.document
                    .span(SourceRange::new(
                        field.value_range().start(),
                        field.value_range().end(),
                    ))
                    .ok()
            })
            .unwrap_or_else(|| self.source.clone());
        ImageCompileError::InvalidField {
            image: self.image.to_owned(),
            field,
            reason,
            span,
        }
    }

    fn unsupported(&self, field: &ImageDeclField) -> Result<(), ImageCompileError> {
        let span = self
            .document
            .span(SourceRange::new(
                field.name_range().start(),
                field.name_range().end(),
            ))
            .map_err(|source| ImageCompileError::InvalidSourceRange {
                image: self.image.to_owned(),
                source,
            })?;
        Err(ImageCompileError::UnsupportedField {
            image: self.image.to_owned(),
            field: field.name().to_owned(),
            span,
        })
    }
}

fn lower_image(
    declaration: &EntityDeclItem,
    body: &ImageDeclBody,
    source: SourceSpan,
    document: &SourceDocument,
) -> Result<BundleImageObject, ImageCompileError> {
    let id = declaration.id().body();
    let fields = ImageFields::try_new(id, source, document, body)?;
    validate_image_fields(&fields)?;
    let asset = declaration_public_id(fields.required("asset")?, DeclarationIdentityFamily::Asset)
        .map(|asset| asset.as_str().to_owned())
        .ok_or_else(|| fields.invalid("asset", "expected an `asset.*` entity reference"))?;
    let placement = image_stage_placement(&fields)?;
    let bounds = image_design_bounds(&fields, &placement)?;
    Ok(BundleImageObject {
        id: id.to_owned(),
        asset,
        target: fields
            .get("target")
            .and_then(public_id)
            .map(|id| id.as_str().to_owned()),
        layer: fields
            .get("layer")
            .and_then(|value| declaration_public_id(value, DeclarationIdentityFamily::Layer))
            .map(|id| id.as_str().to_owned()),
        view: None,
        containing_scroll_region: None,
        bounds,
        placement: Some(placement),
        fit: image_fit(&fields),
        alignment: image_alignment(&fields),
        playback: image_playback(&fields),
        transform: image_transform(&fields),
        depth_milli: fields
            .get("depth")
            .and_then(number)
            .and_then(rounded_i32)
            .unwrap_or_default(),
        opacity_milli: image_opacity(&fields)?,
        actions: image_actions(&fields)?,
        params: image_params(&fields)?,
        proxies: image_proxies(&fields)?,
        visible: fields.get("visible").and_then(boolean).unwrap_or(true),
    })
}

fn validate_image_fields(fields: &ImageFields<'_>) -> Result<(), ImageCompileError> {
    for field in fields.fields.values().copied() {
        let name = field.name();
        let value = field.value();
        let valid = match name {
            "asset" => declaration_public_id(value, DeclarationIdentityFamily::Asset).is_some(),
            "target" | "proxy.id" => public_id(value).is_some(),
            "layer" | "proxy.layer" => {
                declaration_public_id(value, DeclarationIdentityFamily::Layer).is_some()
            }
            "action" => declaration_public_id(value, DeclarationIdentityFamily::Action).is_some(),
            "actions" => {
                matches!(value, Expr::BracketSeq(values) if values.iter().all(|value| declaration_public_id(value, DeclarationIdentityFamily::Action).is_some()))
            }
            "x" | "y" | "width" | "height" | "size.width" | "size.height" | "margin.top"
            | "margin.right" | "margin.bottom" | "margin.left" | "transform.tx"
            | "transform.ty" => px_milli(value).is_some(),
            "position" => anchor(value).is_some(),
            "object_anchor" => keyword(value)
                .as_deref()
                .and_then(StageAnchor::from_keyword)
                .is_some(),
            "scale" => keyword(value)
                .as_deref()
                .and_then(StageScalePolicy::from_keyword)
                .is_some(),
            "safe_area" | "visible" | "proxy.hit_test" => boolean(value).is_some(),
            "fit" => matches!(
                keyword(value).as_deref(),
                Some("contain" | "cover" | "stretch" | "intrinsic")
            ),
            "alignment.x" => alignment(value, "x").is_some(),
            "alignment.y" => alignment(value, "y").is_some(),
            "playback.start" | "playback.paused_at" | "playback.local_time" => {
                duration_millis(value).is_some()
            }
            "playback.rate" | "transform.m11" | "transform.m12" | "transform.m21"
            | "transform.m22" => milli(value).is_some(),
            "depth" | "proxy.depth" => number(value).and_then(rounded_i32).is_some(),
            "opacity" => image_opacity(fields).is_ok(),
            "proxy.type" | "proxy.role" => keyword(value).is_some(),
            name if name.starts_with("param.") || name.starts_with("proxy.param.") => {
                image_param(value).is_some()
            }
            _ => return fields.unsupported(field),
        };
        if !valid {
            return Err(fields.invalid(name, image_field_expectation(name)));
        }
    }
    Ok(())
}

fn image_field_expectation(name: &str) -> &'static str {
    match name {
        "asset" => "expected an `asset.*` entity reference",
        "target" | "proxy.id" => "expected a public entity reference",
        "layer" | "proxy.layer" => "expected a `layer.*` entity reference",
        "action" => "expected an `action.*` entity reference",
        "actions" => "expected a bracket sequence of `action.*` entity references",
        "x" | "y" | "width" | "height" | "size.width" | "size.height" | "margin.top"
        | "margin.right" | "margin.bottom" | "margin.left" | "transform.tx" | "transform.ty" => {
            "expected a px value"
        }
        "position" => "expected `anchor(<stage-anchor>)`",
        "object_anchor" => "expected a stage-anchor keyword",
        "scale" => "expected a stage-scale keyword",
        "safe_area" | "visible" | "proxy.hit_test" => "expected a boolean",
        "fit" => "expected contain, cover, stretch, or intrinsic",
        "alignment.x" | "alignment.y" => "expected an alignment keyword or unitless number",
        "playback.start" | "playback.paused_at" | "playback.local_time" => "expected a duration",
        "playback.rate" | "transform.m11" | "transform.m12" | "transform.m21" | "transform.m22" => {
            "expected a unitless or percent value"
        }
        "depth" | "proxy.depth" => "expected a unitless number",
        "opacity" => "expected a value from 0 to 1",
        "proxy.type" | "proxy.role" => "expected a keyword or string",
        _ => "unsupported retained image value",
    }
}

fn image_stage_placement(fields: &ImageFields<'_>) -> Result<StagePlacement, ImageCompileError> {
    if fields.get("position").is_none() {
        return Ok(StagePlacement::absolute(StageRect::new(
            required_px(fields, "x")?,
            required_px(fields, "y")?,
            non_negative_size(fields, "width", required_px(fields, "width")?)?,
            non_negative_size(fields, "height", required_px(fields, "height")?)?,
        )));
    }
    if ["x", "y", "width", "height"]
        .iter()
        .any(|name| fields.get(name).is_some())
    {
        return Err(fields.invalid(
            "position",
            "cannot mix anchored placement with x/y/width/height",
        ));
    }
    if fields.get("scale.x").is_some() || fields.get("scale.y").is_some() {
        return Err(fields.invalid("scale", "independent stage scale axes are not supported"));
    }
    let anchor = fields
        .get("position")
        .and_then(anchor)
        .ok_or_else(|| fields.invalid("position", "expected `anchor(<stage-anchor>)`"))?;
    let object_anchor = fields
        .get("object_anchor")
        .and_then(keyword)
        .as_deref()
        .and_then(StageAnchor::from_keyword)
        .unwrap_or(anchor);
    let width = non_negative_size(fields, "size.width", required_px(fields, "size.width")?)?;
    let height = non_negative_size(fields, "size.height", required_px(fields, "size.height")?)?;
    let margins = StageInsets::new(
        optional_px(fields, "margin.top"),
        optional_px(fields, "margin.right"),
        optional_px(fields, "margin.bottom"),
        optional_px(fields, "margin.left"),
    );
    let scale = fields
        .get("scale")
        .and_then(keyword)
        .as_deref()
        .and_then(StageScalePolicy::from_keyword)
        .unwrap_or(StageScalePolicy::Design);
    Ok(
        StagePlacement::anchor(anchor, object_anchor, StageSize::new(width, height))
            .with_margins(margins)
            .with_scale_policy(scale)
            .with_safe_area(fields.get("safe_area").and_then(boolean).unwrap_or(false)),
    )
}

fn image_design_bounds(
    fields: &ImageFields<'_>,
    placement: &StagePlacement,
) -> Result<BundleImageObjectBounds, ImageCompileError> {
    let resolved = placement
        .resolve(StagePlacementContext::new(
            LayoutSize::new(1280.0, 720.0),
            LayoutSize::new(1280.0, 720.0),
        ))
        .map_err(|error| ImageCompileError::Placement {
            image: fields.image.to_owned(),
            reason: error.to_string(),
            span: fields.source.clone(),
        })?;
    Ok(BundleImageObjectBounds {
        x_milli: f32_to_i32_milli(resolved.design_bbox.origin.x),
        y_milli: f32_to_i32_milli(resolved.design_bbox.origin.y),
        width_milli: f32_to_u32_milli(resolved.design_bbox.size.width),
        height_milli: f32_to_u32_milli(resolved.design_bbox.size.height),
    })
}

fn image_fit(fields: &ImageFields<'_>) -> BundleImageObjectFit {
    match fields.get("fit").and_then(keyword).as_deref() {
        Some("cover") => BundleImageObjectFit::Cover,
        Some("stretch") => BundleImageObjectFit::Stretch,
        Some("intrinsic") => BundleImageObjectFit::Intrinsic,
        _ => BundleImageObjectFit::Contain,
    }
}

fn image_alignment(fields: &ImageFields<'_>) -> BundleImageObjectAlignment {
    BundleImageObjectAlignment {
        x_milli: fields
            .get("alignment.x")
            .and_then(|value| alignment(value, "x"))
            .unwrap_or(500),
        y_milli: fields
            .get("alignment.y")
            .and_then(|value| alignment(value, "y"))
            .unwrap_or(500),
    }
}

fn alignment(value: &Expr, axis: &str) -> Option<i32> {
    if let Some(value) = keyword(value) {
        match (axis, value.as_str()) {
            ("x", "left" | "start") | ("y", "top" | "start") => return Some(0),
            ("x" | "y", "center" | "middle") => return Some(500),
            ("x", "right" | "end") | ("y", "bottom" | "end") => return Some(1_000),
            _ => {}
        }
    }
    let value = number(value)?;
    rounded_i32(if (0.0..=1.0).contains(&value) {
        value * 1_000.0
    } else {
        value.clamp(0.0, 1_000.0)
    })
}

fn image_playback(fields: &ImageFields<'_>) -> BundleImageObjectPlayback {
    BundleImageObjectPlayback {
        start_time_millis: fields
            .get("playback.start")
            .and_then(duration_millis)
            .unwrap_or_default(),
        rate_milli: fields
            .get("playback.rate")
            .and_then(milli)
            .and_then(|value| u32::try_from(value.max(0)).ok())
            .unwrap_or(1_000),
        paused_at_millis: fields.get("playback.paused_at").and_then(duration_millis),
        pinned_local_time_millis: fields.get("playback.local_time").and_then(duration_millis),
    }
}

fn image_transform(fields: &ImageFields<'_>) -> BundleImageObjectTransform {
    BundleImageObjectTransform {
        m11_milli: fields.get("transform.m11").and_then(milli).unwrap_or(1_000),
        m12_milli: fields
            .get("transform.m12")
            .and_then(milli)
            .unwrap_or_default(),
        m21_milli: fields
            .get("transform.m21")
            .and_then(milli)
            .unwrap_or_default(),
        m22_milli: fields.get("transform.m22").and_then(milli).unwrap_or(1_000),
        tx_milli: fields
            .get("transform.tx")
            .and_then(px_milli)
            .unwrap_or_default(),
        ty_milli: fields
            .get("transform.ty")
            .and_then(px_milli)
            .unwrap_or_default(),
    }
}

fn image_opacity(fields: &ImageFields<'_>) -> Result<u16, ImageCompileError> {
    let Some(value) = fields.get("opacity") else {
        return Ok(1_000);
    };
    let value = match value {
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Percent,
        }) => unit_number(raw, UnitNumberSuffix::Percent).map(|value| value * 10.0),
        value => number(value).map(|value| value * 1_000.0),
    }
    .and_then(rounded_i32)
    .and_then(|value| u16::try_from(value).ok())
    .filter(|value| *value <= 1_000)
    .ok_or_else(|| fields.invalid("opacity", "expected a value from 0 to 1"))?;
    Ok(value)
}

fn image_actions(fields: &ImageFields<'_>) -> Result<Vec<String>, ImageCompileError> {
    let mut actions = Vec::new();
    if let Some(value) = fields.get("action") {
        actions.push(
            declaration_public_id(value, DeclarationIdentityFamily::Action)
                .map(|id| id.as_str().to_owned())
                .ok_or_else(|| fields.invalid("action", "expected an action entity reference"))?,
        );
    }
    if let Some(value) = fields.get("actions") {
        let Expr::BracketSeq(values) = value else {
            return Err(fields.invalid("actions", "expected a bracket sequence of action IDs"));
        };
        for value in values {
            actions.push(
                declaration_public_id(value, DeclarationIdentityFamily::Action)
                    .map(|id| id.as_str().to_owned())
                    .ok_or_else(|| {
                        fields.invalid("actions", "expected a bracket sequence of action IDs")
                    })?,
            );
        }
    }
    Ok(actions)
}

fn image_params(
    fields: &ImageFields<'_>,
) -> Result<BTreeMap<String, BundleImageObjectParam>, ImageCompileError> {
    fields
        .fields
        .iter()
        .filter(|(name, _)| name.starts_with("param."))
        .map(|(name, field)| {
            image_param(field.value())
                .map(|value| ((*name).to_owned(), value))
                .ok_or_else(|| fields.invalid(*name, "unsupported retained parameter value"))
        })
        .collect()
}

fn image_proxies(
    fields: &ImageFields<'_>,
) -> Result<Vec<BundleImageObjectProxy>, ImageCompileError> {
    let Some(id) = fields.get("proxy.id") else {
        return Ok(Vec::new());
    };
    let id = public_id(id)
        .map(|id| id.as_str().to_owned())
        .ok_or_else(|| fields.invalid("proxy.id", "expected a public entity reference"))?;
    let params = fields
        .fields
        .iter()
        .filter_map(|(name, field)| {
            name.strip_prefix("proxy.param.")
                .map(|name| (name, field.value()))
        })
        .map(|(name, value)| {
            image_param(value)
                .map(|value| (name.to_owned(), value))
                .ok_or_else(|| {
                    fields.invalid(
                        format!("proxy.param.{name}"),
                        "unsupported retained parameter value",
                    )
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(vec![BundleImageObjectProxy {
        id,
        type_name: fields.get("proxy.type").and_then(keyword),
        role: fields.get("proxy.role").and_then(keyword),
        layer: fields
            .get("proxy.layer")
            .and_then(|value| declaration_public_id(value, DeclarationIdentityFamily::Layer))
            .map(|id| id.as_str().to_owned()),
        depth_milli: fields
            .get("proxy.depth")
            .and_then(number)
            .and_then(rounded_i32),
        hit_test: fields
            .get("proxy.hit_test")
            .and_then(boolean)
            .unwrap_or_default(),
        params,
    }])
}

fn image_param(value: &Expr) -> Option<BundleImageObjectParam> {
    match value {
        Expr::Literal(Literal::Bool(value)) => Some(BundleImageObjectParam::Bool { value: *value }),
        Expr::Literal(Literal::Int(value)) => value
            .magnitude()
            .ok()
            .and_then(|value| i64::try_from(value).ok())
            .map(|value| BundleImageObjectParam::Integer { value }),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => integer(expr)
            .and_then(i64::checked_neg)
            .map(|value| BundleImageObjectParam::Integer { value }),
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Percent,
        }) => unit_number(raw, UnitNumberSuffix::Percent)
            .and_then(|value| rounded_i32(value * 10.0))
            .map(|value| BundleImageObjectParam::Milli { value }),
        Expr::EntityRef(_) => public_id(value).map(|value| BundleImageObjectParam::Id {
            value: value.as_str().to_owned(),
        }),
        Expr::Literal(Literal::String(value)) => Some(BundleImageObjectParam::Text {
            value: value.clone(),
        }),
        Expr::Path(path) => Some(BundleImageObjectParam::Text {
            value: path.as_label().to_owned(),
        }),
        _ => None,
    }
}

fn required_px(fields: &ImageFields<'_>, name: &'static str) -> Result<i32, ImageCompileError> {
    fields.required(name).and_then(|value| {
        px_milli(value).ok_or_else(|| fields.invalid(name, "expected a px value"))
    })
}

fn optional_px(fields: &ImageFields<'_>, name: &str) -> i32 {
    fields.get(name).and_then(px_milli).unwrap_or_default()
}

fn non_negative_size(
    fields: &ImageFields<'_>,
    name: &'static str,
    value: i32,
) -> Result<u32, ImageCompileError> {
    u32::try_from(value).map_err(|_| fields.invalid(name, "size must not be negative"))
}

fn anchor(value: &Expr) -> Option<StageAnchor> {
    let Expr::Call(call) = value else {
        return None;
    };
    if call.callee().dotted_selector_label().as_deref() != Some("anchor") {
        return None;
    }
    let [CallArg::Positional(value)] = call.args() else {
        return None;
    };
    StageAnchor::from_keyword(keyword(value)?.as_str())
}

fn public_id(value: &Expr) -> Option<PublicId> {
    match value {
        Expr::EntityRef(reference) => PublicId::try_new(reference.canonical_body()).ok(),
        _ => None,
    }
}

fn declaration_public_id(value: &Expr, family: DeclarationIdentityFamily) -> Option<PublicId> {
    let id = public_id(value)?;
    family.validate_public_id(&id).ok()?;
    Some(id)
}

fn keyword(value: &Expr) -> Option<String> {
    match value {
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        Expr::Path(path) => Some(path.as_label().to_owned()),
        Expr::ShortVariant(name) => Some(name.as_str().to_owned()),
        _ => None,
    }
}

fn boolean(value: &Expr) -> Option<bool> {
    match value {
        Expr::Literal(Literal::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn integer(value: &Expr) -> Option<i64> {
    match value {
        Expr::Literal(Literal::Int(value)) => value
            .magnitude()
            .ok()
            .and_then(|value| i64::try_from(value).ok()),
        _ => None,
    }
}

fn number(value: &Expr) -> Option<f64> {
    match value {
        Expr::Literal(Literal::Int(value)) => value.magnitude().ok()?.to_f64(),
        Expr::Literal(Literal::Float { raw, suffix }) => {
            let raw = suffix.map_or(raw.as_str(), |suffix| {
                raw.strip_suffix(suffix.as_str()).unwrap_or(raw.as_str())
            });
            raw.replace('_', "").parse().ok()
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => number(expr).map(|value| -value),
        _ => None,
    }
}

fn unit_number(raw: &str, suffix: UnitNumberSuffix) -> Option<f64> {
    raw.strip_suffix(suffix.as_str())?
        .trim()
        .replace('_', "")
        .parse()
        .ok()
}

fn px_milli(value: &Expr) -> Option<i32> {
    if let Expr::Unary {
        op: UnaryOp::Neg,
        expr,
    } = value
    {
        return px_milli(expr)?.checked_neg();
    }
    let Expr::Literal(Literal::UnitNumber {
        raw,
        suffix: UnitNumberSuffix::Px,
    }) = value
    else {
        return None;
    };
    rounded_i32(unit_number(raw, UnitNumberSuffix::Px)? * 1_000.0)
}

fn milli(value: &Expr) -> Option<i32> {
    match value {
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Percent,
        }) => rounded_i32(unit_number(raw, UnitNumberSuffix::Percent)? * 10.0),
        _ => rounded_i32(number(value)? * 1_000.0),
    }
}

fn duration_millis(value: &Expr) -> Option<u64> {
    let millis = match value {
        Expr::Literal(Literal::Duration { amount, unit }) => {
            let amount = amount.replace('_', "").parse::<f64>().ok()?;
            amount
                * match unit {
                    DurationUnit::Nanos => 0.000_001,
                    DurationUnit::Micros => 0.001,
                    DurationUnit::Millis => 1.0,
                    DurationUnit::Seconds => 1_000.0,
                    DurationUnit::Minutes => 60_000.0,
                    DurationUnit::Hours => 3_600_000.0,
                }
        }
        _ => number(value)?,
    };
    millis.round().clamp(0.0, u64::MAX.to_f64()?).to_u64()
}

fn rounded_i32(value: f64) -> Option<i32> {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
        .to_i32()
}

fn f32_to_i32_milli(value: f32) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    (f64::from(value) * 1_000.0)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
        .to_i32()
        .unwrap_or(0)
}

fn f32_to_u32_milli(value: f32) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    (f64::from(value.max(0.0)) * 1_000.0)
        .round()
        .clamp(0.0, f64::from(u32::MAX))
        .to_u32()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_hir::{lower::lower_document_to_hir, project::HirProjectModule};
    use arcweft_lang_syntax::{
        ast::module_path::CanonicalModulePath,
        parser::{ParseOptions, parse_document_with_source},
    };
    use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
    use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn lowers_typed_image_without_reparsing_source_text() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://image").expect("source ID"),
                SourceName::path("main.arcw"),
                r"
pub image @image.glass_bg {
    asset = @asset:.glass_bg
    x = 0px
    y = 0px
    width = 1280px
    height = 720px
    opacity = 75%
}
",
            )
            .expect("document"),
        );
        let module = CanonicalModulePath::crate_root();
        let syntax = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
        assert!(syntax.errors().is_empty());
        let hir = lower_document_to_hir(document.as_ref(), syntax.typed_tree())
            .expect("source lowers to HIR");
        let hir_project = HirProject::new(
            "local.arcweft.image-test",
            vec![
                HirProjectModule::try_new(module.clone(), document.identity().clone(), hir)
                    .expect("HIR module"),
            ],
        )
        .expect("HIR project");
        let project = ProjectSources::new(
            PathBuf::from("arcw.toml"),
            PathBuf::from("."),
            PackageSpec {
                id: PackageId::new("local.arcweft.image-test").expect("package"),
                version: PackageVersion::new("0.0.0").expect("version"),
            },
            BuildSpec::default(),
            Arc::new(
                SourceDocument::try_new(
                    SourceDocumentId::try_new("arcweft-test://image-manifest")
                        .expect("manifest source ID"),
                    SourceName::path("arcw.toml"),
                    "",
                )
                .expect("manifest document"),
            ),
            vec![ProjectSourceFile::new(
                module,
                PathBuf::from("main.arcw"),
                document,
                [],
            )],
        )
        .expect("project");

        let catalog = lower_project_images(&hir_project, &project).expect("image catalog");
        let [image] = catalog.objects() else {
            panic!("one image expected");
        };
        assert_eq!(image.id, "image.glass_bg");
        assert_eq!(image.asset, "asset.glass_bg");
        assert_eq!(image.opacity_milli, 750);
    }
}
