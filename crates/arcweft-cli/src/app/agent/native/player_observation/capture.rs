//! Retention and painter-ordered debug attachments for one prepared frame.

use super::{
    AgentImageFrameStore, AgentObservedObject, ExitCode, HitRect, NativeAgentRuntimeState,
    PlayerPreparedFrame, PreparedFrame, agent_object_id_color, agent_view_prepared_text_root_id,
};
use arcweft_agent_protocol::rich_text::{AgentRichTextElementKind, AgentRichTextElementRef};
use arcweft_glyphon::{PreparedGlyph, PreparedGlyphSource, PreparedTextItem};
use arcweft_render_text::RichTextRange;
use arcweft_render_wgpu::geometry::PreparedTextOwner;
use arcweft_render_wgpu::offscreen::{
    CaptureAttachment, CaptureCropPolicy, CaptureRegion, CaptureRequest, CaptureScope,
    PreparedTextSelection, PreparedTextSelectionError,
};
use num_traits::ToPrimitive;
use thiserror::Error;

#[derive(Debug, Error)]
enum PlayerTextCaptureSelectionError {
    #[error("rich-text object `{object_id}` has no matching prepared-text owner")]
    MissingOwner { object_id: String },
    #[error("rich-text object `{object_id}` matches {owner_count} prepared-text owners")]
    AmbiguousOwner {
        object_id: String,
        owner_count: usize,
    },
    #[error("rich-text object `{object_id}` references missing prepared-text item {text_index}")]
    MissingPreparedTextItem { object_id: String, text_index: u32 },
    #[error(
        "rich-text object `{object_id}` range {start}..{end} precedes owner origin {source_origin}"
    )]
    RangeBeforeOwner {
        object_id: String,
        start: usize,
        end: usize,
        source_origin: usize,
    },
    #[error("rich-text object `{object_id}` has invalid range {start}..{end}")]
    InvalidRange {
        object_id: String,
        start: usize,
        end: usize,
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

pub(super) fn capture_player_observation_frame(
    runtime: &mut NativeAgentRuntimeState,
    prepared: &PlayerPreparedFrame,
    objects: &[AgentObservedObject],
) -> Result<AgentImageFrameStore, ExitCode> {
    let color_capture = runtime
        .shared_capture
        .capture(&prepared.frame, &CaptureRequest::whole_frame_color())
        .map_err(|error| {
            eprintln!("error: player-backed observe color capture failed: {error}");
            ExitCode::FAILURE
        })?;
    let capture_regions = player_capture_regions(&prepared.frame, objects)?;
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
    AgentImageFrameStore::from_shared_captures(&color_capture, debug_capture.as_ref()).map_err(
        |error| {
            eprintln!("error: player-backed capture retention failed: {error}");
            ExitCode::FAILURE
        },
    )
}

fn player_capture_regions(
    prepared: &PreparedFrame,
    objects: &[AgentObservedObject],
) -> Result<Vec<CaptureRegion>, ExitCode> {
    let mut ordered = objects
        .iter()
        .enumerate()
        .filter(|(_, object)| object.visible)
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(source_index, object)| {
        player_capture_region_order(prepared, object, *source_index)
    });
    ordered
        .into_iter()
        .map(|(_, object)| player_capture_region(prepared, object))
        .collect::<Result<Vec<_>, _>>()
        .map(|regions| regions.into_iter().flatten().collect())
}

fn player_capture_region(
    prepared: &PreparedFrame,
    object: &AgentObservedObject,
) -> Result<Option<CaptureRegion>, ExitCode> {
    let prepared_text = match player_prepared_text_capture_selection(prepared, object) {
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
    prepared: &PreparedFrame,
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
    let owner = player_prepared_text_owner(prepared, object)?;
    let item = prepared.text.get(owner.text).ok_or_else(|| {
        PlayerTextCaptureSelectionError::MissingPreparedTextItem {
            object_id: object.id.clone(),
            text_index: owner.text.index(),
        }
    })?;
    let local_range = player_local_text_range(object, reference, owner)?;
    let glyph_indices = player_selected_glyph_indices(object, item, reference, local_range)?;
    if glyph_indices.is_empty() {
        return Err(PlayerTextCaptureSelectionError::NoPreparedGlyphs {
            object_id: object.id.clone(),
        });
    }
    PreparedTextSelection::try_new(owner.text, glyph_indices)
        .map(Some)
        .map_err(|source| PlayerTextCaptureSelectionError::InvalidSelection {
            object_id: object.id.clone(),
            source,
        })
}

fn player_prepared_text_owner<'a>(
    prepared: &'a PreparedFrame,
    object: &AgentObservedObject,
) -> Result<&'a PreparedTextOwner, PlayerTextCaptureSelectionError> {
    let owners = prepared
        .prepared_text_owners()
        .iter()
        .filter(|owner| player_object_belongs_to_text_owner(object, owner))
        .collect::<Vec<_>>();
    let owner = match owners.as_slice() {
        [owner] => *owner,
        [] => {
            return Err(PlayerTextCaptureSelectionError::MissingOwner {
                object_id: object.id.clone(),
            });
        }
        _ => {
            return Err(PlayerTextCaptureSelectionError::AmbiguousOwner {
                object_id: object.id.clone(),
                owner_count: owners.len(),
            });
        }
    };
    Ok(owner)
}

fn player_local_text_range(
    object: &AgentObservedObject,
    reference: &AgentRichTextElementRef,
    owner: &PreparedTextOwner,
) -> Result<RichTextRange, PlayerTextCaptureSelectionError> {
    let Some(start) = reference.range.start.checked_sub(owner.source_origin) else {
        return Err(PlayerTextCaptureSelectionError::RangeBeforeOwner {
            object_id: object.id.clone(),
            start: reference.range.start,
            end: reference.range.end,
            source_origin: owner.source_origin,
        });
    };
    let Some(end) = reference.range.end.checked_sub(owner.source_origin) else {
        return Err(PlayerTextCaptureSelectionError::RangeBeforeOwner {
            object_id: object.id.clone(),
            start: reference.range.start,
            end: reference.range.end,
            source_origin: owner.source_origin,
        });
    };
    if start > end {
        return Err(PlayerTextCaptureSelectionError::InvalidRange {
            object_id: object.id.clone(),
            start,
            end,
        });
    }
    Ok(RichTextRange::new(start, end))
}

fn player_selected_glyph_indices(
    object: &AgentObservedObject,
    item: &PreparedTextItem,
    reference: &AgentRichTextElementRef,
    local_range: RichTextRange,
) -> Result<Vec<u32>, PlayerTextCaptureSelectionError> {
    item.glyphs
        .iter()
        .enumerate()
        .filter(|(_, glyph)| player_glyph_matches(reference, glyph, local_range))
        .map(|(index, _)| {
            u32::try_from(index).map_err(|_| PlayerTextCaptureSelectionError::GlyphIndexOverflow {
                object_id: object.id.clone(),
                glyph_index: index,
            })
        })
        .collect()
}

fn player_glyph_matches(
    reference: &AgentRichTextElementRef,
    glyph: &PreparedGlyph,
    local_range: RichTextRange,
) -> bool {
    match reference.kind {
        AgentRichTextElementKind::GlyphCluster => {
            matches!(glyph.source, PreparedGlyphSource::Body { .. })
                && usize::try_from(glyph.cluster_index).ok() == Some(reference.index)
                && local_range.start <= glyph.source_range.start
                && glyph.source_range.end <= local_range.end
        }
        AgentRichTextElementKind::Ruby => match glyph.source {
            PreparedGlyphSource::Body { .. } => {
                local_range.start <= glyph.source_range.start
                    && glyph.source_range.end <= local_range.end
            }
            PreparedGlyphSource::Ruby { ruby_index, .. } => {
                usize::try_from(ruby_index).ok() == Some(reference.index)
            }
        },
        AgentRichTextElementKind::TextPage
        | AgentRichTextElementKind::TextLine
        | AgentRichTextElementKind::TextRun
        | AgentRichTextElementKind::TextGlyph
        | AgentRichTextElementKind::TextObjectProxy => false,
    }
}

fn player_capture_region_order(
    prepared: &PreparedFrame,
    object: &AgentObservedObject,
    source_index: usize,
) -> (u8, usize, usize, usize) {
    if object.role == "image" {
        let image_index = object
            .entity
            .as_deref()
            .and_then(|entity| prepared.images.iter().position(|image| image.id == entity))
            .unwrap_or(source_index);
        return (0, image_index, 0, source_index);
    }
    if let Some((owner_index, owner)) = prepared
        .prepared_text_owners()
        .iter()
        .enumerate()
        .find(|(_, owner)| player_object_belongs_to_text_owner(object, owner))
    {
        let phase = match owner.kind {
            arcweft_render_wgpu::geometry::PreparedTextOwnerKind::View { .. } => 1,
            arcweft_render_wgpu::geometry::PreparedTextOwnerKind::Control => 3,
            arcweft_render_wgpu::geometry::PreparedTextOwnerKind::DialogueView { .. } => 4,
        };
        let element_order = prepared.text.get(owner.text).map_or(0, |item| {
            player_text_element_paint_order(owner, item, object)
        });
        return (phase, owner_index, element_order, source_index);
    }
    let semantic_index = prepared
        .semantics
        .as_slice()
        .iter()
        .position(|node| node.target().id().as_str() == object.id)
        .unwrap_or(source_index);
    (2, semantic_index, 0, source_index)
}

fn player_object_belongs_to_text_owner(
    object: &AgentObservedObject,
    owner: &arcweft_render_wgpu::geometry::PreparedTextOwner,
) -> bool {
    match owner.kind {
        arcweft_render_wgpu::geometry::PreparedTextOwnerKind::DialogueView {
            dialogue,
            entry,
            ..
        } => {
            let root = format!("object.dialogue.{dialogue}.{entry}");
            object.id == root || object.id.starts_with(&format!("{root}."))
        }
        arcweft_render_wgpu::geometry::PreparedTextOwnerKind::View { .. } => {
            let Some(root) = agent_view_prepared_text_root_id(owner) else {
                return false;
            };
            object.id == root || object.id.starts_with(&format!("{root}."))
        }
        arcweft_render_wgpu::geometry::PreparedTextOwnerKind::Control => {
            object.id == owner.semantic_id.as_str()
        }
    }
}

fn player_text_element_paint_order(
    owner: &arcweft_render_wgpu::geometry::PreparedTextOwner,
    item: &arcweft_glyphon::PreparedTextItem,
    object: &AgentObservedObject,
) -> usize {
    let Some(reference) = &object.rich_text_ref else {
        return 0;
    };
    let Some(local_start) = reference.range.start.checked_sub(owner.source_origin) else {
        return 0;
    };
    let Some(local_end) = reference.range.end.checked_sub(owner.source_origin) else {
        return 0;
    };
    let local_range = arcweft_render_text::RichTextRange::new(local_start, local_end);
    match reference.kind {
        arcweft_agent_protocol::rich_text::AgentRichTextElementKind::GlyphCluster => item
            .layout
            .glyphs
            .iter()
            .position(|glyph| glyph_is_cluster_member(glyph, local_range, reference.index))
            .map_or(0, |index| index.saturating_add(1)),
        arcweft_agent_protocol::rich_text::AgentRichTextElementKind::Ruby => item
            .layout
            .ruby
            .iter()
            .position(|ruby| ruby.base_range == local_range)
            .map_or(0, |index| {
                item.layout
                    .glyphs
                    .len()
                    .saturating_add(index)
                    .saturating_add(1)
            }),
        arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextPage
        | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextLine
        | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextRun
        | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextGlyph
        | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextObjectProxy => 0,
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

fn glyph_is_cluster_member(
    glyph: &arcweft_text_layout::TextLayoutGlyph,
    cluster_range: arcweft_render_text::RichTextRange,
    cluster_index: usize,
) -> bool {
    usize::try_from(glyph.cluster_index).ok() == Some(cluster_index)
        && cluster_range.start <= glyph.source_range.start
        && glyph.source_range.end <= cluster_range.end
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
