//! Retention and painter-ordered debug attachments for one prepared frame.

use super::{
    AgentImageFrameStore, AgentObservedObject, ExitCode, HitRect, NativeAgentRuntimeState,
    PlayerPreparedFrame, PreparedFrame, agent_object_id_color,
};
use arcweft_render_wgpu::offscreen::{
    CaptureAttachment, CaptureCropPolicy, CaptureRegion, CaptureRequest, CaptureScope,
};
use num_traits::ToPrimitive;

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
        .filter(|(_, object)| object.visible && player_object_has_capture_region(prepared, object))
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(source_index, object)| {
        player_capture_region_order(prepared, object, *source_index)
    });
    ordered
        .into_iter()
        .map(|(_, object)| {
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
            Ok(CaptureRegion::new(
                id,
                HitRect::new(x, y, width, height),
                agent_object_id_color(&object.id),
            ))
        })
        .collect()
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
            arcweft_render_wgpu::geometry::PreparedTextOwnerKind::TextBox { .. } => 4,
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
        arcweft_render_wgpu::geometry::PreparedTextOwnerKind::TextBox {
            textbox,
            entry,
            part: arcweft_render_wgpu::geometry::PreparedTextBoxPart::Body,
            ..
        } => {
            let root = format!("object.dialogue.{textbox}.{entry}");
            object.id == root || object.id.starts_with(&format!("{root}."))
        }
        arcweft_render_wgpu::geometry::PreparedTextOwnerKind::TextBox {
            part: arcweft_render_wgpu::geometry::PreparedTextBoxPart::Speaker,
            ..
        } => false,
        arcweft_render_wgpu::geometry::PreparedTextOwnerKind::View { .. } => object
            .entity
            .as_deref()
            .is_some_and(|entity| entity == owner.semantic_id.as_str()),
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
            .position(|glyph| {
                glyph.source_range == local_range
                    && usize::try_from(glyph.cluster_index).ok() == Some(reference.index)
            })
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

fn player_object_has_capture_region(
    prepared: &PreparedFrame,
    object: &AgentObservedObject,
) -> bool {
    let Some(reference) = &object.rich_text_ref else {
        return object.role != "text";
    };
    match reference.kind {
        arcweft_agent_protocol::rich_text::AgentRichTextElementKind::GlyphCluster
        | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::Ruby => {
            player_prepared_text_element_is_visible(prepared, reference)
        }
        arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextPage
        | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextLine
        | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextRun
        | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextGlyph
        | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextObjectProxy => false,
    }
}

fn player_prepared_text_element_is_visible(
    prepared: &PreparedFrame,
    reference: &arcweft_agent_protocol::rich_text::AgentRichTextElementRef,
) -> bool {
    prepared.prepared_text_owners().iter().any(|owner| {
        let Some(item) = prepared.text.get(owner.text) else {
            return false;
        };
        let Some(start) = reference.range.start.checked_sub(owner.source_origin) else {
            return false;
        };
        let Some(end) = reference.range.end.checked_sub(owner.source_origin) else {
            return false;
        };
        let local_range = arcweft_render_text::RichTextRange::new(start, end);
        match reference.kind {
            arcweft_agent_protocol::rich_text::AgentRichTextElementKind::GlyphCluster => item
                .layout
                .glyphs
                .iter()
                .enumerate()
                .filter(|(_, glyph)| {
                    glyph.source_range == local_range
                        && usize::try_from(glyph.cluster_index).ok() == Some(reference.index)
                })
                .any(|(index, _)| {
                    item.paint
                        .glyphs
                        .get(index)
                        .is_some_and(text_paint_is_visible)
                }),
            arcweft_agent_protocol::rich_text::AgentRichTextElementKind::Ruby => {
                let mut paint_offset = item.layout.glyphs.len();
                item.layout.ruby.iter().any(|ruby| {
                    let matches = ruby.base_range == local_range;
                    let visible = matches
                        && item
                            .paint
                            .glyphs
                            .get(paint_offset..paint_offset.saturating_add(ruby.glyphs.len()))
                            .is_some_and(|paint| paint.iter().any(text_paint_is_visible));
                    paint_offset = paint_offset.saturating_add(ruby.glyphs.len());
                    visible
                })
            }
            arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextPage
            | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextLine
            | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextRun
            | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextGlyph
            | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextObjectProxy => false,
        }
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
