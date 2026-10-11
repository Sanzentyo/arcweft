//! Retention and painter-ordered debug attachments for one prepared frame.

use super::{
    AgentImageFrameStore, AgentObservedObject, ExitCode, HitRect, NativeAgentRuntimeState,
    PlayerPreparedFrame, PreparedFrame, PreparedTextObservationOwner, agent_object_id_color,
};
use arcweft_agent_protocol::rich_text::{AgentRichTextElementKind, AgentRichTextElementRef};
use arcweft_agent_protocol::text_context::{
    AgentTextBodyCapture, AgentTextContextCatalog, AgentTextContextContent, AgentTextContextError,
    AgentTextContextScope,
};
use arcweft_glyphon::{PreparedGlyphSource, PreparedTextItem};
use arcweft_render_wgpu::geometry::PreparedTextOwner;
use arcweft_render_wgpu::offscreen::{
    CaptureAttachment, CaptureCropPolicy, CaptureRegion, CaptureRequest, CaptureScope,
    PreparedTextSelection, PreparedTextSelectionError,
};
use arcweft_text_model::{PreparedTextContext, PreparedTextOwnerKind, ProjectedTextRangeError};
use num_traits::ToPrimitive;
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

#[cfg(test)]
mod tests;

#[derive(Debug, Error)]
enum PlayerTextCaptureSelectionError {
    #[error(transparent)]
    Context(#[from] AgentTextContextError),
    #[error("text context {owner} does not describe the actual prepared owner/item")]
    PreparedOwnerMismatch { owner: u32 },
    #[error("rich-text object `{object_id}` references missing prepared-text item {text_index}")]
    MissingPreparedTextItem { object_id: String, text_index: u32 },
    #[error("rich-text object `{object_id}` has an invalid projected range: {source}")]
    Range {
        object_id: String,
        #[source]
        source: ProjectedTextRangeError,
    },
    #[error("rich-text object `{object_id}` resolves to no prepared glyphs")]
    NoPreparedGlyphs { object_id: String },
    #[error("rich-text object `{object_id}` prepared glyph index {glyph_index} exceeds u32")]
    GlyphIndexOverflow {
        object_id: String,
        glyph_index: usize,
    },
    #[error("rich-text object `{object_id}` has an invalid prepared glyph selection: {source}")]
    InvalidSelection {
        object_id: String,
        #[source]
        source: PreparedTextSelectionError,
    },
}

/// A transport reference never admits capture data. This adapter binds every
/// issued row once to the current renderer owner and retained context. The
/// Ruby lookup is derived from those two admitted projections, not a new frame.
struct PlayerTextCaptureContexts<'frame> {
    catalog: &'frame AgentTextContextCatalog,
    owners: BTreeMap<u32, PlayerTextCaptureOwner<'frame>>,
}

struct PlayerTextCaptureOwner<'frame> {
    prepared: &'frame PreparedTextOwner,
    item: &'frame PreparedTextItem,
    ruby_indices: BTreeMap<usize, u32>,
}

impl<'frame> PlayerTextCaptureContexts<'frame> {
    fn bind(
        prepared: &'frame PreparedFrame,
        catalog: &'frame AgentTextContextCatalog,
        expected_scope: &AgentTextContextScope,
    ) -> Result<Self, PlayerTextCaptureSelectionError> {
        if catalog.scope() != expected_scope {
            return Err(AgentTextContextError::ScopeMismatch.into());
        }
        let mut owners = BTreeMap::new();
        for entry in catalog.entries() {
            let ordinal = entry.owner_ordinal;
            let actual = usize::try_from(ordinal)
                .ok()
                .and_then(|index| prepared.prepared_text_owners().get(index))
                .ok_or(PlayerTextCaptureSelectionError::PreparedOwnerMismatch { owner: ordinal })?;
            let item = prepared.text.get(actual.text).ok_or_else(|| {
                PlayerTextCaptureSelectionError::MissingPreparedTextItem {
                    object_id: entry.owner.root_id.clone(),
                    text_index: actual.text.index(),
                }
            })?;
            let identity = PreparedTextObservationOwner::new(actual);
            if entry.owner.root_id != identity.root_id()
                || entry.owner.semantic_id != actual.semantic_id.as_str()
                || entry.owner.parent_id != actual.parent_id.as_ref().map(ToString::to_string)
                || entry.owner.prepared_text != actual.text.index()
                || entry.owner.kind != actual.kind
            {
                return Err(PlayerTextCaptureSelectionError::PreparedOwnerMismatch {
                    owner: ordinal,
                });
            }
            let mut ruby_indices = BTreeMap::new();
            match (&actual.context, &entry.content) {
                (
                    PreparedTextContext::DisplayStage { context: actual },
                    AgentTextContextContent::DisplayStage {
                        context: issued,
                        layout,
                    },
                ) if Arc::ptr_eq(actual, issued) && Arc::ptr_eq(layout, &item.layout) => {
                    let mut layout_ruby = BTreeMap::new();
                    for ruby in &item.layout.ruby {
                        let range = actual.ranges().source_to_display(ruby.base_range).map_err(
                            |source| PlayerTextCaptureSelectionError::Range {
                                object_id: entry.owner.root_id.clone(),
                                source,
                            },
                        )?;
                        layout_ruby.insert(
                            (ruby.owner_node, range.start, range.end, ruby.text.as_str()),
                            ruby.ruby_index,
                        );
                    }
                    for (index, ruby) in actual
                        .frame()
                        .display_map
                        .ruby_annotations
                        .iter()
                        .enumerate()
                    {
                        if let Some(prepared_index) = layout_ruby.get(&(
                            ruby.owner_node,
                            ruby.base_range.start,
                            ruby.base_range.end,
                            ruby.ruby.as_str(),
                        )) {
                            ruby_indices.insert(index, *prepared_index);
                        }
                    }
                }
                (
                    PreparedTextContext::Document { context: actual },
                    AgentTextContextContent::PreparedDocument {
                        context: issued,
                        text,
                        layout,
                    },
                ) if actual == issued
                    && text == &item.interaction.text
                    && Arc::ptr_eq(layout, &item.layout) =>
                {
                    for ruby in &item.layout.ruby {
                        if let Ok(index) = usize::try_from(ruby.ruby_index) {
                            ruby_indices.insert(index, ruby.ruby_index);
                        }
                    }
                }
                _ => {
                    return Err(PlayerTextCaptureSelectionError::PreparedOwnerMismatch {
                        owner: ordinal,
                    });
                }
            }
            owners.insert(
                ordinal,
                PlayerTextCaptureOwner {
                    prepared: actual,
                    item,
                    ruby_indices,
                },
            );
        }
        Ok(Self { catalog, owners })
    }

    fn owner(
        &self,
        object: &AgentObservedObject,
    ) -> Result<Option<&PlayerTextCaptureOwner<'frame>>, PlayerTextCaptureSelectionError> {
        object.validate_text_context(self.catalog)?;
        let Some(reference) = &object.text_context else {
            return Ok(None);
        };
        self.catalog.resolve(reference)?;
        self.owners
            .get(&reference.owner())
            .map(Some)
            .ok_or_else(|| PlayerTextCaptureSelectionError::PreparedOwnerMismatch {
                owner: reference.owner(),
            })
    }
}

pub(super) fn capture_player_observation_frame(
    runtime: &mut NativeAgentRuntimeState,
    prepared: &PlayerPreparedFrame,
    objects: &[AgentObservedObject],
    catalog: &AgentTextContextCatalog,
    expected_scope: &AgentTextContextScope,
) -> Result<AgentImageFrameStore, ExitCode> {
    let contexts = PlayerTextCaptureContexts::bind(&prepared.frame, catalog, expected_scope)
        .map_err(|error| {
            eprintln!("error: player capture context binding failed: {error}");
            ExitCode::FAILURE
        })?;
    let color_capture = runtime
        .shared_capture
        .capture(&prepared.frame, &CaptureRequest::whole_frame_color())
        .map_err(|error| {
            eprintln!("error: player-backed observe color capture failed: {error}");
            ExitCode::FAILURE
        })?;
    let capture_regions = player_capture_regions(&prepared.frame, &contexts, objects)?;
    let debug_capture = if capture_regions.is_empty() {
        None
    } else {
        Some(
            runtime
                .shared_capture
                .capture(
                    &prepared.frame,
                    &CaptureRequest::new(
                        [CaptureAttachment::ObjectId, CaptureAttachment::Mask],
                        CaptureScope::Regions(capture_regions),
                        CaptureCropPolicy::FullFrame,
                    ),
                )
                .map_err(|error| {
                    eprintln!("error: player-backed observe debug capture failed: {error}");
                    ExitCode::FAILURE
                })?,
        )
    };
    let mut diagnostics = Vec::new();
    super::append_fx_diagnostics(
        &mut diagnostics,
        &prepared.frame.fx_diagnostics,
        expected_scope.tick,
    );
    AgentImageFrameStore::from_shared_captures(
        expected_scope.clone(),
        &color_capture,
        debug_capture.as_ref(),
        diagnostics,
    )
    .map_err(|error| {
        eprintln!("error: player-backed capture retention failed: {error}");
        ExitCode::FAILURE
    })
}

fn player_capture_regions(
    prepared: &PreparedFrame,
    contexts: &PlayerTextCaptureContexts<'_>,
    objects: &[AgentObservedObject],
) -> Result<Vec<CaptureRegion>, ExitCode> {
    let mut ordered = objects
        .iter()
        .enumerate()
        .filter(|(_, object)| object.visible)
        .collect::<Vec<_>>();
    for (_, object) in &ordered {
        contexts.owner(object).map_err(|error| {
            eprintln!("error: player capture context: {error}");
            ExitCode::FAILURE
        })?;
    }
    ordered.sort_by_key(|(source_index, object)| {
        player_capture_region_order(prepared, contexts, object, *source_index)
    });
    ordered
        .into_iter()
        .map(|(_, object)| player_capture_region(prepared, contexts, object))
        .collect::<Result<Vec<_>, _>>()
        .map(|regions| regions.into_iter().flatten().collect())
}

fn player_capture_region(
    prepared: &PreparedFrame,
    contexts: &PlayerTextCaptureContexts<'_>,
    object: &AgentObservedObject,
) -> Result<Option<CaptureRegion>, ExitCode> {
    let prepared_text = match player_prepared_text_capture_selection(contexts, object) {
        Ok(selection) => selection,
        Err(error) => {
            eprintln!("error: player capture selection failed: {error}");
            return Err(ExitCode::FAILURE);
        }
    };
    if object.rich_text_ref.is_some() && prepared_text.is_none() {
        return Ok(None);
    }
    if object.rich_text_ref.is_some()
        && prepared_text.as_ref().is_some_and(|selection| {
            !player_prepared_text_selection_is_visible(prepared, selection)
        })
    {
        return Ok(None);
    }
    if object.rich_text_ref.is_none() && object.role == "text" {
        return Ok(None);
    }
    let id = arcweft_id::PublicId::try_new(&object.id).map_err(|error| {
        eprintln!(
            "error: observed object id `{}` cannot identify a capture region: {error}",
            object.id
        );
        ExitCode::FAILURE
    })?;
    let x = object.bbox.x.to_f32().ok_or_else(|| {
        eprintln!("error: capture region x coordinate is not representable as f32");
        ExitCode::FAILURE
    })?;
    let y = object.bbox.y.to_f32().ok_or_else(|| {
        eprintln!("error: capture region y coordinate is not representable as f32");
        ExitCode::FAILURE
    })?;
    let width = object.bbox.width.to_f32().ok_or_else(|| {
        eprintln!("error: capture region width is not representable as f32");
        ExitCode::FAILURE
    })?;
    let height = object.bbox.height.to_f32().ok_or_else(|| {
        eprintln!("error: capture region height is not representable as f32");
        ExitCode::FAILURE
    })?;
    let bounds = HitRect::new(x, y, width, height);
    let object_id_rgba = agent_object_id_color(&object.id);
    let Some(selection) = prepared_text else {
        return Ok(Some(CaptureRegion::new(id, bounds, object_id_rgba)));
    };
    Ok(Some(CaptureRegion::prepared_text(
        id,
        bounds,
        object_id_rgba,
        selection,
    )))
}

fn player_prepared_text_capture_selection(
    contexts: &PlayerTextCaptureContexts<'_>,
    object: &AgentObservedObject,
) -> Result<Option<PreparedTextSelection>, PlayerTextCaptureSelectionError> {
    let Some(reference) = object.rich_text_ref.as_ref() else {
        return Ok(None);
    };
    if matches!(
        reference.kind,
        AgentRichTextElementKind::TextPage
            | AgentRichTextElementKind::TextLine
            | AgentRichTextElementKind::TextRun
            | AgentRichTextElementKind::TextGlyph
            | AgentRichTextElementKind::TextObjectProxy
    ) {
        return Ok(None);
    }
    let owner =
        contexts
            .owner(object)?
            .ok_or_else(|| AgentTextContextError::MissingObjectContext {
                object_id: object.id.clone(),
            })?;
    let context = object.text_context.as_ref().ok_or_else(|| {
        AgentTextContextError::MissingObjectContext {
            object_id: object.id.clone(),
        }
    })?;
    let body = contexts.catalog.resolve(context)?.capture_body(reference)?;
    let glyph_indices = player_selected_glyph_indices(object, owner, reference, &body)?;
    if glyph_indices.is_empty() {
        return Err(PlayerTextCaptureSelectionError::NoPreparedGlyphs {
            object_id: object.id.clone(),
        });
    }
    PreparedTextSelection::try_new(owner.prepared.text, glyph_indices)
        .map(Some)
        .map_err(|source| PlayerTextCaptureSelectionError::InvalidSelection {
            object_id: object.id.clone(),
            source,
        })
}

fn player_selected_glyph_indices(
    object: &AgentObservedObject,
    owner: &PlayerTextCaptureOwner<'_>,
    reference: &AgentRichTextElementRef,
    body: &AgentTextBodyCapture<'_>,
) -> Result<Vec<u32>, PlayerTextCaptureSelectionError> {
    owner
        .item
        .glyphs
        .iter()
        .enumerate()
        .filter(|(_, glyph)| match glyph.source {
            PreparedGlyphSource::Body { glyph_index } => body.contains_glyph(glyph_index),
            PreparedGlyphSource::Ruby { ruby_index, .. } => {
                reference.kind == AgentRichTextElementKind::Ruby
                    && owner.ruby_indices.get(&reference.index).copied() == Some(ruby_index)
            }
        })
        .map(|(index, _)| {
            u32::try_from(index).map_err(|_| PlayerTextCaptureSelectionError::GlyphIndexOverflow {
                object_id: object.id.clone(),
                glyph_index: index,
            })
        })
        .collect()
}

/// Bounds are overwritten by actual alpha. Ruby retains annotation ownership;
/// body cluster partitions overwrite its base so a later crop can join exact
/// cluster members without treating a whole Ruby base as one glyph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PlayerTextCapturePaintOrder {
    Bounds,
    Ruby(usize),
    Cluster(usize),
}

fn player_capture_region_order(
    prepared: &PreparedFrame,
    contexts: &PlayerTextCaptureContexts<'_>,
    object: &AgentObservedObject,
    source_index: usize,
) -> (u8, usize, PlayerTextCapturePaintOrder, usize) {
    if object.role == "image" {
        let image_index = object
            .entity
            .as_deref()
            .and_then(|entity| prepared.images.iter().position(|image| image.id == entity))
            .unwrap_or(source_index);
        return (
            0,
            image_index,
            PlayerTextCapturePaintOrder::Bounds,
            source_index,
        );
    }
    if let Some(reference) = &object.text_context
        && let Some(owner) = contexts.owners.get(&reference.owner())
    {
        let phase = match owner.prepared.kind {
            PreparedTextOwnerKind::View { .. } => 1,
            PreparedTextOwnerKind::Control => 3,
            PreparedTextOwnerKind::DialogueView { .. } => 4,
        };
        let element_order = player_text_element_paint_order(object);
        return (
            phase,
            usize::try_from(reference.owner()).expect("bound owner ordinal"),
            element_order,
            source_index,
        );
    }
    let semantic_index = prepared
        .semantics
        .as_slice()
        .iter()
        .position(|node| node.target().id().as_str() == object.id)
        .unwrap_or(source_index);
    (
        2,
        semantic_index,
        PlayerTextCapturePaintOrder::Bounds,
        source_index,
    )
}

fn player_text_element_paint_order(object: &AgentObservedObject) -> PlayerTextCapturePaintOrder {
    let Some(reference) = &object.rich_text_ref else {
        return PlayerTextCapturePaintOrder::Bounds;
    };
    match reference.kind {
        AgentRichTextElementKind::Ruby => PlayerTextCapturePaintOrder::Ruby(reference.index),
        AgentRichTextElementKind::GlyphCluster => {
            PlayerTextCapturePaintOrder::Cluster(reference.index)
        }
        AgentRichTextElementKind::TextPage
        | AgentRichTextElementKind::TextLine
        | AgentRichTextElementKind::TextRun
        | AgentRichTextElementKind::TextGlyph
        | AgentRichTextElementKind::TextObjectProxy => PlayerTextCapturePaintOrder::Bounds,
    }
}

fn player_prepared_text_selection_is_visible(
    prepared: &PreparedFrame,
    selection: &PreparedTextSelection,
) -> bool {
    let Some(item) = prepared.text.get(selection.text()) else {
        return false;
    };
    selection.glyph_indices().iter().copied().any(|index| {
        usize::try_from(index)
            .ok()
            .and_then(|index| item.paint.glyphs.get(index))
            .is_some_and(text_paint_is_visible)
    })
}

fn text_paint_is_visible(paint: &arcweft_glyphon::TextGlyphPaint) -> bool {
    paint.visible
        && paint.opacity_milli > 0
        && paint.transform.resolved().opacity().value().get() > 0.0
        && paint
            .masks
            .iter()
            .all(|mask| mask.effective_coverage().value().get() > 0.0)
}
