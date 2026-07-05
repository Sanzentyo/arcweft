use super::{
    NativeEffectExecution, NativeFrameContentBBox, NativeFrameElement, NativeFrameElementBounds,
    NativeGlyphClusterMetadata, NativeGlyphPlacement, NativeRubyElementGeometry, NativeVisualPage,
    NativeVisualRun, NativeWindowError, apply_presentation_effects_to_placement,
    apply_presentation_effects_to_placement_with_execution,
    apply_presentation_to_placement_with_effects, apply_shaped_horizontal_origins_to_placements,
    glyph_presentation_affine, intersect_display_range, native_float_bbox, observe_layout_shaders,
    presentation_affine, resolve_shader_filter, shaped_horizontal_glyph_metrics,
    surface_extent_f32, usize_to_f32_saturating, valid_display_range,
};
use arcweft_render_text::{
    LineDisplayFrame, RichTextDisplayMap, RichTextEffectPhase, RichTextPresentation, RichTextRange,
    RichTextTextRun, RichTextTransformOrigin, RichTextWritingMode,
};
use arcweft_text_layout::{
    GlyphOrientation, GlyphVerticalForm, LaidOutGlyph, LaidOutText, LayoutPoint, LayoutRect,
    LayoutSize, TextLayoutConfig, layout_frame,
};
use glyphon::Vector;
use std::collections::BTreeMap;
use std::ops::Range;

pub(super) fn visual_page_from_range(
    frame: &LineDisplayFrame,
    page_index: usize,
    page_range: Range<usize>,
    time_seconds: f32,
    effects: &mut NativeEffectExecution<'_>,
) -> Option<NativeVisualPage> {
    let page_layout = layout_page_range(
        frame,
        page_range.clone(),
        TextLayoutConfig {
            origin: LayoutPoint::new(0.0, 0.0),
            size: LayoutSize::new(720.0, 360.0),
            effect_time_seconds: time_seconds,
            ..TextLayoutConfig::default()
        },
    )
    .ok()?;
    if page_layout.frame.text.is_empty() {
        return None;
    }
    let runs = native_visual_runs_from_layout(&page_layout, page_range.start);
    let glyphs = native_glyph_placements_from_layout(
        &page_layout,
        &runs,
        page_range.start,
        time_seconds,
        effects,
    );
    let shaders = runs
        .iter()
        .flat_map(|run| run.presentation.shaders.iter().map(resolve_shader_filter))
        .collect();
    observe_layout_shaders(
        effects,
        &page_layout.layout,
        page_layout.layout.ruby.iter().filter_map(|ruby| {
            page_layout
                .ruby_indices
                .get(ruby.ruby_index)
                .and_then(|index| frame.display_map.ruby_annotations.get(*index))
                .map(|ruby| &ruby.presentation)
        }),
    );
    Some(NativeVisualPage {
        page_index,
        text: page_layout.frame.text,
        runs,
        glyphs,
        shaders,
    })
}

pub(super) struct NativePageLayout {
    pub(super) frame: LineDisplayFrame,
    pub(super) page_start: usize,
    pub(super) config: TextLayoutConfig,
    pub(super) layout: LaidOutText,
    pub(super) text_run_indices: Vec<usize>,
    pub(super) ruby_indices: Vec<usize>,
}

pub(super) fn layout_page_range(
    frame: &LineDisplayFrame,
    page_range: Range<usize>,
    config: TextLayoutConfig,
) -> Result<NativePageLayout, NativeWindowError> {
    let page_start = page_range.start;
    let (page_frame, text_run_indices, ruby_indices) = page_local_layout_frame(frame, page_range)?;
    let layout = layout_frame(&page_frame, config)
        .map_err(|error| NativeWindowError::TextLayout(error.to_string()))?;
    Ok(NativePageLayout {
        frame: page_frame,
        page_start,
        config,
        layout,
        text_run_indices,
        ruby_indices,
    })
}

pub(super) fn layout_page_range_with_selected_text(
    frame: &LineDisplayFrame,
    page_range: Range<usize>,
    config: TextLayoutConfig,
    selected_text: &[Range<usize>],
) -> Result<NativePageLayout, NativeWindowError> {
    let page_start = page_range.start;
    let (mut page_frame, text_run_indices, ruby_indices) =
        page_local_layout_frame(frame, page_range)?;
    let (text_runs, text_run_indices) = split_text_runs_for_capture_selection(
        &page_frame.display_map.text_runs,
        &text_run_indices,
        selected_text,
        &page_frame.text,
    );
    page_frame.display_map.text_runs = text_runs;
    let layout = layout_frame(&page_frame, config)
        .map_err(|error| NativeWindowError::TextLayout(error.to_string()))?;
    Ok(NativePageLayout {
        frame: page_frame,
        page_start,
        config,
        layout,
        text_run_indices,
        ruby_indices,
    })
}

pub(super) fn split_text_runs_for_capture_selection(
    runs: &[RichTextTextRun],
    source_indices: &[usize],
    selected_text: &[Range<usize>],
    text: &str,
) -> (Vec<RichTextTextRun>, Vec<usize>) {
    let mut split_runs = Vec::new();
    let mut split_source_indices = Vec::new();
    for (run, source_index) in runs.iter().zip(source_indices.iter().copied()) {
        let run_range = run.range.start..run.range.end;
        let mut boundaries = vec![run_range.start, run_range.end];
        boundaries.extend(selected_text.iter().flat_map(|range| {
            [
                range.start.clamp(run_range.start, run_range.end),
                range.end.clamp(run_range.start, run_range.end),
            ]
        }));
        boundaries.retain(|offset| *offset <= text.len() && text.is_char_boundary(*offset));
        boundaries.sort_unstable();
        boundaries.dedup();
        split_runs.extend(boundaries.windows(2).filter_map(|window| {
            let start = window[0];
            let end = window[1];
            if start >= end {
                return None;
            }
            let mut split = run.clone();
            split.range = RichTextRange::new(start, end);
            if !selected_text
                .iter()
                .any(|range| range.start <= start && end <= range.end)
            {
                split.presentation = capture_unselected_presentation(&split.presentation);
            }
            split_source_indices.push(source_index);
            Some(split)
        }));
    }
    (split_runs, split_source_indices)
}

pub(super) fn capture_unselected_presentation(
    presentation: &RichTextPresentation,
) -> RichTextPresentation {
    let mut out = presentation.clone();
    out.transform = None;
    out.shaders.clear();
    out.effects.retain(|effect| {
        matches!(
            effect.phase,
            RichTextEffectPhase::BeforeLayout | RichTextEffectPhase::LayoutTransform
        )
    });
    out.object_proxies.clear();
    out.opacity = None;
    out
}

pub(super) fn page_local_layout_frame(
    frame: &LineDisplayFrame,
    page_range: Range<usize>,
) -> Result<(LineDisplayFrame, Vec<usize>, Vec<usize>), NativeWindowError> {
    let text = frame
        .text
        .get(page_range.clone())
        .ok_or(NativeWindowError::EmptyPages)?
        .to_owned();
    if text.is_empty() {
        return Err(NativeWindowError::EmptyPages);
    }

    let mut text_run_indices = Vec::new();
    let text_runs = frame
        .display_map
        .text_runs
        .iter()
        .enumerate()
        .filter_map(|(index, run)| {
            let range = intersect_display_range(run.range, &page_range)?;
            text_run_indices.push(index);
            let mut run = run.clone();
            run.range =
                RichTextRange::new(range.start - page_range.start, range.end - page_range.start);
            Some(run)
        })
        .collect();

    let mut ruby_indices = Vec::new();
    let ruby_annotations = frame
        .display_map
        .ruby_annotations
        .iter()
        .enumerate()
        .filter_map(|(index, annotation)| {
            let base_range = valid_display_range(annotation.base_range, &frame.text)?;
            if base_range.start < page_range.start || base_range.end > page_range.end {
                return None;
            }
            ruby_indices.push(index);
            let mut annotation = annotation.clone();
            annotation.base_range = RichTextRange::new(
                base_range.start - page_range.start,
                base_range.end - page_range.start,
            );
            Some(annotation)
        })
        .collect();

    Ok((
        LineDisplayFrame {
            line: frame.line.clone(),
            callee: frame.callee.clone(),
            speaker_label: frame.speaker_label.clone(),
            text,
            base_styles: frame.base_styles.clone(),
            default_inline_failure_policy: frame.default_inline_failure_policy.clone(),
            style_contributions: frame.style_contributions.clone(),
            nodes: Vec::new(),
            display_map: RichTextDisplayMap {
                text_runs,
                ruby_annotations,
                controls: Vec::new(),
                host_events: Vec::new(),
            },
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        },
        text_run_indices,
        ruby_indices,
    ))
}

pub(super) fn native_visual_runs_from_layout(
    page_layout: &NativePageLayout,
    page_start: usize,
) -> Vec<NativeVisualRun> {
    page_layout
        .layout
        .runs
        .iter()
        .filter_map(|run| {
            let source_run_index = *page_layout.text_run_indices.get(run.run_index)?;
            Some(NativeVisualRun {
                source_run_index,
                range: (page_start + run.range.start)..(page_start + run.range.end),
                local_range: run.range.start..run.range.end,
                presentation: run.presentation.clone(),
            })
        })
        .collect()
}

pub(super) fn native_glyph_placements_from_layout(
    page_layout: &NativePageLayout,
    runs: &[NativeVisualRun],
    page_start: usize,
    time_seconds: f32,
    effects: &mut NativeEffectExecution<'_>,
) -> Vec<NativeGlyphPlacement> {
    let mut run_counts = BTreeMap::<usize, usize>::new();
    for glyph in &page_layout.layout.glyphs {
        *run_counts.entry(glyph.run_index).or_default() += 1;
    }

    let mut next_glyph_indices = BTreeMap::<usize, usize>::new();
    let mut placements = page_layout
        .layout
        .glyphs
        .iter()
        .filter_map(|glyph| {
            let source_run_index = *page_layout.text_run_indices.get(glyph.run_index)?;
            let glyph_index = next_glyph_indices.entry(glyph.run_index).or_default();
            let range = (page_start + glyph.range.start)..(page_start + glyph.range.end);
            let mut placement = NativeGlyphPlacement {
                run_index: source_run_index,
                glyph_index: *glyph_index,
                range,
                x: glyph.origin.x,
                y: glyph.origin.y,
                rotate_degrees: glyph_orientation_degrees(glyph.orientation),
                skew_x_degrees: 0.0,
                skew_y_degrees: 0.0,
                affine_origin: None,
                affine_target: None,
                vertical_form: glyph.vertical_form,
                scale_x: 1.0,
                scale_y: 1.0,
                opacity: 1.0,
                color: None,
            };
            *glyph_index += 1;
            let run = runs
                .iter()
                .find(|run| run.source_run_index == source_run_index)?;
            apply_presentation_to_placement_with_effects(
                &page_layout.frame.line.0,
                run,
                *run_counts.get(&glyph.run_index).unwrap_or(&1),
                time_seconds,
                effects,
                &mut placement,
            );
            Some(placement)
        })
        .collect::<Vec<_>>();
    let shaped_metrics = shaped_horizontal_glyph_metrics(page_layout);
    apply_shaped_horizontal_origins_to_placements(
        &mut placements,
        &page_layout.layout,
        &shaped_metrics,
    );
    placements
}

pub(super) fn native_element_bounds_from_layout_at(
    page_layout: &NativePageLayout,
    width: u32,
    height: u32,
    time_seconds: f32,
    mut effects: Option<&mut NativeEffectExecution<'_>>,
) -> Vec<NativeFrameElementBounds> {
    let transformed =
        native_transformed_glyph_bounds(page_layout, time_seconds, effects.as_deref_mut());
    let mut bounds = native_text_run_bounds_from_layout(page_layout, &transformed, width, height);
    bounds.extend(native_text_proxy_bounds_from_layout(
        page_layout,
        &transformed,
        width,
        height,
    ));
    bounds.extend(native_glyph_cluster_bounds_from_layout(
        page_layout,
        &transformed,
        width,
        height,
    ));
    bounds.extend(native_ruby_bounds_from_layout(
        page_layout,
        width,
        height,
        time_seconds,
        effects,
    ));
    bounds
}

pub(super) fn native_text_run_bounds_from_layout(
    page_layout: &NativePageLayout,
    transformed: &NativeTransformedGlyphBounds,
    width: u32,
    height: u32,
) -> Vec<NativeFrameElementBounds> {
    page_layout
        .layout
        .runs
        .iter()
        .filter_map(|run| {
            let index = *page_layout.text_run_indices.get(run.run_index)?;
            let bounds = transformed
                .run_bounds
                .get(&run.run_index)
                .copied()
                .unwrap_or(run.bounds);
            Some(NativeFrameElementBounds {
                element: NativeFrameElement::TextRun { index },
                bbox: native_bbox_from_layout_rect(bounds, width, height)?,
                glyph: None,
                ruby: None,
            })
        })
        .collect()
}

pub(super) fn native_text_proxy_bounds_from_layout(
    page_layout: &NativePageLayout,
    transformed: &NativeTransformedGlyphBounds,
    width: u32,
    height: u32,
) -> Vec<NativeFrameElementBounds> {
    page_layout
        .layout
        .runs
        .iter()
        .filter_map(|run| {
            let source_run_index = *page_layout.text_run_indices.get(run.run_index)?;
            let source_run = page_layout.frame.display_map.text_runs.get(run.run_index)?;
            let bounds = transformed
                .run_bounds
                .get(&run.run_index)
                .copied()
                .unwrap_or(run.bounds);
            Some(
                source_run
                    .presentation
                    .object_proxies
                    .iter()
                    .enumerate()
                    .filter_map(move |(proxy_index, _)| {
                        Some(NativeFrameElementBounds {
                            element: NativeFrameElement::TextObjectProxy {
                                run_index: source_run_index,
                                proxy_index,
                            },
                            bbox: native_bbox_from_layout_rect(bounds, width, height)?,
                            glyph: None,
                            ruby: None,
                        })
                    }),
            )
        })
        .flatten()
        .collect()
}

pub(super) fn native_glyph_cluster_bounds_from_layout(
    page_layout: &NativePageLayout,
    transformed: &NativeTransformedGlyphBounds,
    width: u32,
    height: u32,
) -> Vec<NativeFrameElementBounds> {
    page_layout
        .layout
        .glyphs
        .iter()
        .enumerate()
        .filter_map(|(index, glyph)| {
            let range_start = page_layout.page_start + glyph.range.start;
            let range_end = page_layout.page_start + glyph.range.end;
            let bounds = transformed
                .glyph_bounds
                .get(index)
                .copied()
                .unwrap_or(glyph.bounds);
            Some(NativeFrameElementBounds {
                element: NativeFrameElement::GlyphCluster {
                    index,
                    range_start,
                    range_end,
                },
                bbox: native_bbox_from_layout_rect(bounds, width, height)?,
                glyph: Some(NativeGlyphClusterMetadata {
                    orientation: glyph.orientation.into(),
                    vertical_form: glyph.vertical_form.into(),
                }),
                ruby: None,
            })
        })
        .collect()
}

pub(super) fn native_ruby_bounds_from_layout(
    page_layout: &NativePageLayout,
    width: u32,
    height: u32,
    time_seconds: f32,
    mut effects: Option<&mut NativeEffectExecution<'_>>,
) -> Vec<NativeFrameElementBounds> {
    let ruby_bounds_by_index = page_layout
        .layout
        .ruby
        .iter()
        .filter_map(|ruby| {
            let index = *page_layout.ruby_indices.get(ruby.ruby_index)?;
            Some((
                index,
                transformed_ruby_geometry(
                    &page_layout.frame.line.0,
                    &page_layout.frame.text,
                    index,
                    ruby,
                    time_seconds,
                    effects.as_deref_mut(),
                ),
            ))
        })
        .fold(
            BTreeMap::<usize, NativeRubyLayoutGeometry>::new(),
            |mut bounds, (index, geometry)| {
                bounds
                    .entry(index)
                    .and_modify(|existing| *existing = existing.union(geometry))
                    .or_insert(geometry);
                bounds
            },
        );
    ruby_bounds_by_index
        .into_iter()
        .filter_map(|(index, geometry)| {
            let bounds = inflate_layout_rect_asymmetric(geometry.object, 16.0, 16.0, 16.0, 16.0);
            Some(NativeFrameElementBounds {
                element: NativeFrameElement::Ruby { index },
                bbox: native_bbox_from_layout_rect(bounds, width, height)?,
                glyph: None,
                ruby: Some(NativeRubyElementGeometry {
                    base_bbox: native_bbox_from_layout_rect(geometry.base, width, height)?,
                    annotation_bbox: native_bbox_from_layout_rect(
                        geometry.annotation,
                        width,
                        height,
                    )?,
                }),
            })
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
pub(super) struct NativeTransformedGlyphBounds {
    pub(super) glyph_bounds: Vec<LayoutRect>,
    pub(super) run_bounds: BTreeMap<usize, LayoutRect>,
}

pub(super) fn native_transformed_glyph_bounds(
    page_layout: &NativePageLayout,
    time_seconds: f32,
    effects: Option<&mut NativeEffectExecution<'_>>,
) -> NativeTransformedGlyphBounds {
    let mut placements = native_glyph_placements_for_layout_with_effects(
        &page_layout.frame.line.0,
        &page_layout.layout,
        time_seconds,
        effects,
    );
    let shaped_metrics = shaped_horizontal_glyph_metrics(page_layout);
    apply_shaped_horizontal_origins_to_placements(
        &mut placements,
        &page_layout.layout,
        &shaped_metrics,
    );
    let glyph_bounds = page_layout
        .layout
        .glyphs
        .iter()
        .enumerate()
        .zip(placements.iter())
        .map(|((glyph_index, glyph), placement)| {
            transformed_glyph_bounds(
                glyph,
                placement,
                &page_layout.layout,
                shaped_metrics.advance(glyph_index),
            )
        })
        .collect::<Vec<_>>();
    let run_bounds = page_layout
        .layout
        .glyphs
        .iter()
        .zip(glyph_bounds.iter())
        .fold(
            BTreeMap::<usize, LayoutRect>::new(),
            |mut out, (glyph, bounds)| {
                out.entry(glyph.run_index)
                    .and_modify(|existing| *existing = existing.union(*bounds))
                    .or_insert(*bounds);
                out
            },
        );
    NativeTransformedGlyphBounds {
        glyph_bounds,
        run_bounds,
    }
}

pub(super) fn native_glyph_placements_for_layout(
    line_key: &str,
    layout: &LaidOutText,
    time_seconds: f32,
) -> Vec<NativeGlyphPlacement> {
    native_glyph_placements_for_layout_with_effects(line_key, layout, time_seconds, None)
}

pub(super) fn native_glyph_placements_for_layout_with_effects(
    line_key: &str,
    layout: &LaidOutText,
    time_seconds: f32,
    mut effects: Option<&mut NativeEffectExecution<'_>>,
) -> Vec<NativeGlyphPlacement> {
    let glyph_count_by_run =
        layout
            .glyphs
            .iter()
            .fold(BTreeMap::<usize, usize>::new(), |mut counts, glyph| {
                *counts.entry(glyph.run_index).or_default() += 1;
                counts
            });
    let mut glyph_index_by_run = BTreeMap::<usize, usize>::new();
    layout
        .glyphs
        .iter()
        .map(|glyph| {
            let run_glyph_index = glyph_index_by_run.entry(glyph.run_index).or_default();
            let glyph_count = *glyph_count_by_run.get(&glyph.run_index).unwrap_or(&1);
            let mut placement = NativeGlyphPlacement {
                run_index: glyph.run_index,
                glyph_index: *run_glyph_index,
                range: glyph.range.start..glyph.range.end,
                x: glyph.origin.x,
                y: glyph.origin.y,
                rotate_degrees: glyph_orientation_degrees(glyph.orientation),
                skew_x_degrees: 0.0,
                skew_y_degrees: 0.0,
                affine_origin: None,
                affine_target: None,
                vertical_form: glyph.vertical_form,
                scale_x: 1.0,
                scale_y: 1.0,
                opacity: 1.0,
                color: None,
            };
            if let Some(effects) = effects.as_deref_mut() {
                apply_presentation_effects_to_placement_with_execution(
                    line_key,
                    &glyph.presentation,
                    glyph_count,
                    time_seconds,
                    effects,
                    &mut placement,
                );
            } else {
                apply_presentation_effects_to_placement(
                    line_key,
                    &glyph.presentation,
                    glyph_count,
                    time_seconds,
                    &mut placement,
                );
            }
            *run_glyph_index += 1;
            placement
        })
        .collect()
}

pub(super) fn transformed_glyph_bounds(
    glyph: &LaidOutGlyph,
    placement: &NativeGlyphPlacement,
    layout: &LaidOutText,
    width_override: Option<f32>,
) -> LayoutRect {
    let affine = glyph_presentation_affine(placement, glyph, layout);
    let local_left = glyph.bounds.x - glyph.origin.x;
    let local_top = glyph.bounds.y - glyph.origin.y;
    let local_right = local_left + width_override.unwrap_or(glyph.bounds.width);
    let local_bottom = local_top + glyph.bounds.height;
    let corners = [
        transform_glyph_local_point(local_left, local_top, placement, affine),
        transform_glyph_local_point(local_right, local_top, placement, affine),
        transform_glyph_local_point(local_right, local_bottom, placement, affine),
        transform_glyph_local_point(local_left, local_bottom, placement, affine),
    ];
    let min_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    LayoutRect::new(
        min_x,
        min_y,
        (max_x - min_x).max(1.0),
        (max_y - min_y).max(1.0),
    )
}

pub(super) fn transform_glyph_local_point(
    x: f32,
    y: f32,
    placement: &NativeGlyphPlacement,
    affine: Option<[f32; 6]>,
) -> LayoutPoint {
    let [matrix_a, matrix_b, matrix_c, matrix_d, matrix_e, matrix_f] =
        affine.unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    LayoutPoint::new(
        placement.x + matrix_a.mul_add(x, matrix_c.mul_add(y, matrix_e)),
        placement.y + matrix_b.mul_add(x, matrix_d.mul_add(y, matrix_f)),
    )
}

pub(super) fn transformed_ruby_geometry(
    line_key: &str,
    text: &str,
    source_index: usize,
    ruby: &arcweft_text_layout::LaidOutRuby,
    time_seconds: f32,
    mut effects: Option<&mut NativeEffectExecution<'_>>,
) -> NativeRubyLayoutGeometry {
    let base_count = text
        .get(ruby.base_range.start..ruby.base_range.end)
        .map_or(1, |base| base.chars().count().max(1));
    let ruby_count = ruby.ruby.chars().count().max(1);
    let base_ctx = RubySequenceTransformContext {
        line_key,
        presentation: &ruby.presentation,
        source_index,
        writing_mode: ruby.writing_mode,
        glyph_count: base_count,
        time_seconds,
    };
    let annotation_ctx = RubySequenceTransformContext {
        glyph_count: ruby_count,
        ..base_ctx
    };
    let base =
        transformed_ruby_sequence_bounds(ruby.base_bounds, &base_ctx, effects.as_deref_mut());
    let annotation = transformed_ruby_sequence_bounds(ruby.ruby_bounds, &annotation_ctx, effects);
    NativeRubyLayoutGeometry {
        object: base.union(annotation),
        base,
        annotation,
    }
}

#[derive(Clone, Copy)]
pub(super) struct RubySequenceTransformContext<'a> {
    pub(super) line_key: &'a str,
    pub(super) presentation: &'a RichTextPresentation,
    pub(super) source_index: usize,
    pub(super) writing_mode: RichTextWritingMode,
    pub(super) glyph_count: usize,
    pub(super) time_seconds: f32,
}

pub(super) fn transformed_ruby_sequence_bounds(
    bounds: LayoutRect,
    ctx: &RubySequenceTransformContext<'_>,
    mut effects: Option<&mut NativeEffectExecution<'_>>,
) -> LayoutRect {
    let mut out = None;
    for glyph_index in 0..ctx.glyph_count {
        let cell = ruby_sequence_cell(bounds, ctx.writing_mode, ctx.glyph_count, glyph_index);
        let transformed =
            transformed_presentation_rect(ctx, glyph_index, cell, effects.as_deref_mut());
        out = Some(out.map_or(transformed, |existing: LayoutRect| {
            existing.union(transformed)
        }));
    }
    out.unwrap_or(bounds)
}

pub(super) fn ruby_sequence_cell(
    bounds: LayoutRect,
    writing_mode: RichTextWritingMode,
    glyph_count: usize,
    glyph_index: usize,
) -> LayoutRect {
    let glyph_count = usize_to_f32_saturating(glyph_count).max(1.0);
    match writing_mode {
        RichTextWritingMode::HorizontalTb => {
            let width = (bounds.width / glyph_count).max(1.0);
            LayoutRect::new(
                bounds.x + width * usize_to_f32_saturating(glyph_index),
                bounds.y,
                width,
                bounds.height,
            )
        }
        RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
            let height = (bounds.height / glyph_count).max(1.0);
            LayoutRect::new(
                bounds.x,
                bounds.y + height * usize_to_f32_saturating(glyph_index),
                bounds.width,
                height,
            )
        }
    }
}

pub(super) fn transformed_presentation_rect(
    ctx: &RubySequenceTransformContext<'_>,
    glyph_index: usize,
    rect: LayoutRect,
    effects: Option<&mut NativeEffectExecution<'_>>,
) -> LayoutRect {
    let mut placement = NativeGlyphPlacement {
        run_index: ctx.source_index,
        glyph_index,
        range: glyph_index..glyph_index + 1,
        x: rect.x,
        y: rect.y,
        rotate_degrees: 0.0,
        skew_x_degrees: 0.0,
        skew_y_degrees: 0.0,
        affine_origin: None,
        affine_target: None,
        vertical_form: GlyphVerticalForm::None,
        scale_x: 1.0,
        scale_y: 1.0,
        opacity: 1.0,
        color: None,
    };
    if let Some(effects) = effects {
        apply_presentation_effects_to_placement_with_execution(
            ctx.line_key,
            ctx.presentation,
            ctx.glyph_count,
            ctx.time_seconds,
            effects,
            &mut placement,
        );
    } else {
        apply_presentation_effects_to_placement(
            ctx.line_key,
            ctx.presentation,
            ctx.glyph_count,
            ctx.time_seconds,
            &mut placement,
        );
    }
    let affine = presentation_affine(
        &placement,
        0.0,
        layout_rect_transform_pivot(ctx.presentation, rect),
    );
    transformed_local_rect(rect.width, rect.height, &placement, affine)
}

pub(super) fn transformed_local_rect(
    width: f32,
    height: f32,
    placement: &NativeGlyphPlacement,
    affine: Option<[f32; 6]>,
) -> LayoutRect {
    let corners = [
        transform_glyph_local_point(0.0, 0.0, placement, affine),
        transform_glyph_local_point(width, 0.0, placement, affine),
        transform_glyph_local_point(width, height, placement, affine),
        transform_glyph_local_point(0.0, height, placement, affine),
    ];
    let min_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    LayoutRect::new(
        min_x,
        min_y,
        (max_x - min_x).max(1.0),
        (max_y - min_y).max(1.0),
    )
}

pub(super) fn layout_rect_transform_pivot(
    presentation: &RichTextPresentation,
    rect: LayoutRect,
) -> Vector {
    let Some(transform) = &presentation.transform else {
        return Vector::new(0.0, 0.0);
    };
    match transform.origin {
        RichTextTransformOrigin::BaselineStart => Vector::new(0.0, 0.0),
        RichTextTransformOrigin::BaselineCenter
        | RichTextTransformOrigin::Center
        | RichTextTransformOrigin::GlyphCenter => Vector::new(rect.width * 0.5, rect.height * 0.5),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeRubyLayoutGeometry {
    pub(super) object: LayoutRect,
    pub(super) base: LayoutRect,
    pub(super) annotation: LayoutRect,
}

impl NativeRubyLayoutGeometry {
    pub(super) fn union(self, other: Self) -> Self {
        Self {
            object: self.object.union(other.object),
            base: self.base.union(other.base),
            annotation: self.annotation.union(other.annotation),
        }
    }
}

pub(super) fn native_text_layout_config(
    width: u32,
    height: u32,
    left: f32,
    top: f32,
) -> TextLayoutConfig {
    native_text_layout_config_at(width, height, left, top, 0.0)
}

pub(super) fn native_text_layout_config_at(
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    effect_time_seconds: f32,
) -> TextLayoutConfig {
    TextLayoutConfig {
        origin: LayoutPoint::new(left, top),
        size: LayoutSize::new(
            (surface_extent_f32(width) - left).max(1.0),
            (surface_extent_f32(height) - top).max(1.0),
        ),
        effect_time_seconds,
        ..TextLayoutConfig::default()
    }
}

pub(super) fn native_bbox_from_layout_rect(
    rect: LayoutRect,
    width: u32,
    height: u32,
) -> Option<NativeFrameContentBBox> {
    native_float_bbox(rect.x, rect.y, rect.width, rect.height, width, height)
}

pub(super) fn inflate_layout_rect_asymmetric(
    rect: LayoutRect,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
) -> LayoutRect {
    LayoutRect::new(
        rect.x - left,
        rect.y - top,
        rect.width + left + right,
        rect.height + top + bottom,
    )
}

pub(super) const fn glyph_orientation_degrees(orientation: GlyphOrientation) -> f32 {
    match orientation {
        GlyphOrientation::Upright | GlyphOrientation::TextCombineUpright => 0.0,
        GlyphOrientation::SidewaysCw => 90.0,
    }
}
