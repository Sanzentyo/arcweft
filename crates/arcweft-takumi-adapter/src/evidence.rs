use crate::capture::{
    TakumiCaptureFrame, TakumiCaptureRecord, TakumiCompositingCaptureRecord,
    TakumiCompositingGroupId, TakumiEffectOutsets, TakumiPaintNodeId,
};
use crate::metadata::ArcweftNodeMetadata;
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_scene::{UiBlendMode, UiIsolation, UiPrimitiveRange};
use std::fmt::{self, Write as _};

pub const COMPOSITING_EVIDENCE_SCHEMA_VERSION: &str = "arcweft.compositing-capture.v1";

pub fn capture_frame_to_json(frame: &TakumiCaptureFrame) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_field_string(
        &mut out,
        1,
        "schema_version",
        COMPOSITING_EVIDENCE_SCHEMA_VERSION,
        true,
    );
    push_objects(&mut out, frame.records(), true);
    push_groups(&mut out, frame.compositing_records(), false);
    out.push_str("}\n");
    out
}

fn push_objects(out: &mut String, records: &[TakumiCaptureRecord], comma: bool) {
    indent(out, 1);
    out.push_str("\"objects\": [");
    if records.is_empty() {
        out.push(']');
        push_comma_newline(out, comma);
        return;
    }
    out.push('\n');
    for (index, record) in records.iter().enumerate() {
        push_object_record(out, record, index + 1 != records.len());
    }
    indent(out, 1);
    out.push(']');
    push_comma_newline(out, comma);
}

fn push_groups(out: &mut String, records: &[TakumiCompositingCaptureRecord], comma: bool) {
    indent(out, 1);
    out.push_str("\"groups\": [");
    if records.is_empty() {
        out.push(']');
        push_comma_newline(out, comma);
        return;
    }
    out.push('\n');
    for (index, record) in records.iter().enumerate() {
        push_group_record(out, record, index + 1 != records.len());
    }
    indent(out, 1);
    out.push(']');
    push_comma_newline(out, comma);
}

fn push_object_record(out: &mut String, record: &TakumiCaptureRecord, comma: bool) {
    indent(out, 2);
    out.push_str("{\n");
    push_field_string(out, 3, "record_kind", "object", true);
    push_optional_u32(
        out,
        3,
        "paint_node_id",
        record.paint_node_id().map(TakumiPaintNodeId::get),
        true,
    );
    push_optional_u32(
        out,
        3,
        "compositing_group_id",
        record
            .compositing_group_id()
            .map(TakumiCompositingGroupId::get),
        true,
    );
    push_primitive_range(
        out,
        3,
        "primitive_range",
        Some(record.primitive_range()),
        true,
    );
    push_metadata(out, 3, record.metadata(), true);
    push_bounds(out, 3, "layout_bounds", record.layout_bounds(), true);
    push_bounds(out, 3, "primitive_bounds", record.local_bounds(), true);
    push_bounds(out, 3, "visual_bounds", record.visual_bounds(), true);
    push_bounds(out, 3, "hit_bounds", record.hit_bounds(), true);
    push_optional_bounds(out, 3, "clip_bounds", record.clip_bounds(), true);
    push_bounds_array(out, 3, "mask_bounds", record.mask_bounds(), true);
    push_effect_outsets(out, 3, record.effect_outsets(), false);
    indent(out, 2);
    out.push('}');
    push_comma_newline(out, comma);
}

fn push_group_record(out: &mut String, record: &TakumiCompositingCaptureRecord, comma: bool) {
    indent(out, 2);
    out.push_str("{\n");
    push_field_string(out, 3, "record_kind", "compositing_group", true);
    push_field_u32(out, 3, "paint_node_id", record.paint_node_id().get(), true);
    push_field_u32(
        out,
        3,
        "compositing_group_id",
        record.compositing_group_id().get(),
        true,
    );
    push_primitive_range(out, 3, "primitive_range", record.primitive_range(), true);
    push_metadata(out, 3, record.metadata(), true);
    push_field_string(
        out,
        3,
        "isolation",
        &format_isolation(record.isolation()),
        true,
    );
    push_field_string(
        out,
        3,
        "blend_mode",
        &format_blend_mode(record.blend_mode()),
        true,
    );
    push_bounds(out, 3, "layout_bounds", record.layout_bounds(), true);
    push_bounds(out, 3, "visual_bounds", record.visual_bounds(), true);
    push_bounds(out, 3, "hit_bounds", record.hit_bounds(), true);
    push_optional_bounds(out, 3, "clip_bounds", record.clip_bounds(), true);
    push_bounds_array(out, 3, "mask_bounds", record.mask_bounds(), true);
    push_effect_outsets(out, 3, record.effect_outsets(), false);
    indent(out, 2);
    out.push('}');
    push_comma_newline(out, comma);
}

fn push_metadata(out: &mut String, depth: usize, metadata: &ArcweftNodeMetadata, comma: bool) {
    indent(out, depth);
    out.push_str("\"metadata\": {\n");
    push_field_u32(out, depth + 1, "node", metadata.node().0, true);
    push_field_u64(out, depth + 1, "key", metadata.key().0, true);
    push_field_string(
        out,
        depth + 1,
        "kind",
        &format!("{:?}", metadata.kind()),
        true,
    );
    push_field_u32(out, depth + 1, "style", metadata.style().0, true);
    push_optional_u32(out, depth + 1, "view", metadata.view().map(|id| id.0), true);
    push_optional_u32(
        out,
        depth + 1,
        "program",
        metadata.program().map(|id| id.0),
        true,
    );
    push_optional_u32(out, depth + 1, "part", metadata.part().map(|id| id.0), true);
    push_optional_u32(
        out,
        depth + 1,
        "semantic",
        metadata.semantic().map(|id| id.0),
        true,
    );
    push_u32_array(
        out,
        depth + 1,
        "handlers",
        metadata.handlers().iter().map(|handler| handler.0),
        true,
    );
    match metadata.agent() {
        Some(agent) => push_field_string(out, depth + 1, "agent", agent.as_str(), false),
        None => push_null(out, depth + 1, "agent", false),
    }
    indent(out, depth);
    out.push('}');
    push_comma_newline(out, comma);
}

fn push_effect_outsets(out: &mut String, depth: usize, outsets: TakumiEffectOutsets, comma: bool) {
    indent(out, depth);
    out.push_str("\"effect_outsets\": {\n");
    push_field_f32(out, depth + 1, "filter_px", outsets.filter_px, true);
    push_field_f32(
        out,
        depth + 1,
        "backdrop_filter_px",
        outsets.backdrop_filter_px,
        true,
    );
    push_field_f32(out, depth + 1, "mask_px", outsets.mask_px, true);
    push_field_f32(out, depth + 1, "total_px", outsets.total_px, false);
    indent(out, depth);
    out.push('}');
    push_comma_newline(out, comma);
}

fn push_bounds(out: &mut String, depth: usize, name: &str, rect: HitRect, comma: bool) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    out.push_str(": ");
    push_rect_value(out, rect);
    push_comma_newline(out, comma);
}

fn push_optional_bounds(
    out: &mut String,
    depth: usize,
    name: &str,
    rect: Option<HitRect>,
    comma: bool,
) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    out.push_str(": ");
    if let Some(rect) = rect {
        push_rect_value(out, rect);
    } else {
        out.push_str("null");
    }
    push_comma_newline(out, comma);
}

fn push_bounds_array(out: &mut String, depth: usize, name: &str, bounds: &[HitRect], comma: bool) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    out.push_str(": [");
    for (index, rect) in bounds.iter().copied().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        push_rect_value(out, rect);
    }
    out.push(']');
    push_comma_newline(out, comma);
}

fn push_rect_value(out: &mut String, rect: HitRect) {
    out.push('{');
    push_inline_number_field(out, "x", rect.x, true);
    push_inline_number_field(out, "y", rect.y, true);
    push_inline_number_field(out, "width", rect.width, true);
    push_inline_number_field(out, "height", rect.height, false);
    out.push('}');
}

fn push_primitive_range(
    out: &mut String,
    depth: usize,
    name: &str,
    range: Option<UiPrimitiveRange>,
    comma: bool,
) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    out.push_str(": ");
    if let Some(range) = range {
        out.push('{');
        push_inline_u32_field(out, "start", range.start, true);
        push_inline_u32_field(out, "end", range.end, false);
        out.push('}');
    } else {
        out.push_str("null");
    }
    push_comma_newline(out, comma);
}

fn push_field_string(out: &mut String, depth: usize, name: &str, value: &str, comma: bool) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    out.push_str(": ");
    push_json_string(out, value).expect("write to string");
    push_comma_newline(out, comma);
}

fn push_field_u32(out: &mut String, depth: usize, name: &str, value: u32, comma: bool) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    write!(out, ": {value}").expect("write to string");
    push_comma_newline(out, comma);
}

fn push_field_u64(out: &mut String, depth: usize, name: &str, value: u64, comma: bool) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    write!(out, ": {value}").expect("write to string");
    push_comma_newline(out, comma);
}

fn push_optional_u32(out: &mut String, depth: usize, name: &str, value: Option<u32>, comma: bool) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    out.push_str(": ");
    if let Some(value) = value {
        write!(out, "{value}").expect("write to string");
    } else {
        out.push_str("null");
    }
    push_comma_newline(out, comma);
}

fn push_u32_array(
    out: &mut String,
    depth: usize,
    name: &str,
    values: impl Iterator<Item = u32>,
    comma: bool,
) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    out.push_str(": [");
    for (index, value) in values.enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write!(out, "{value}").expect("write to string");
    }
    out.push(']');
    push_comma_newline(out, comma);
}

fn push_field_f32(out: &mut String, depth: usize, name: &str, value: f32, comma: bool) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    out.push_str(": ");
    push_f32(out, value);
    push_comma_newline(out, comma);
}

fn push_null(out: &mut String, depth: usize, name: &str, comma: bool) {
    indent(out, depth);
    push_json_string(out, name).expect("write to string");
    out.push_str(": null");
    push_comma_newline(out, comma);
}

fn push_inline_number_field(out: &mut String, name: &str, value: f32, comma: bool) {
    push_json_string(out, name).expect("write to string");
    out.push_str(": ");
    push_f32(out, value);
    if comma {
        out.push_str(", ");
    }
}

fn push_inline_u32_field(out: &mut String, name: &str, value: u32, comma: bool) {
    push_json_string(out, name).expect("write to string");
    write!(out, ": {value}").expect("write to string");
    if comma {
        out.push_str(", ");
    }
}

fn push_f32(out: &mut String, value: f32) {
    let value = if value.is_finite() { value } else { 0.0 };
    if (value.fract()).abs() <= f32::EPSILON {
        write!(out, "{value:.1}").expect("write to string");
    } else {
        let mut rendered = format!("{value:.3}");
        while rendered.contains('.') && rendered.ends_with('0') {
            rendered.pop();
        }
        if rendered.ends_with('.') {
            rendered.push('0');
        }
        out.push_str(&rendered);
    }
}

fn push_json_string(out: &mut String, value: &str) -> fmt::Result {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            value if value.is_control() => write!(out, "\\u{:04x}", value as u32)?,
            value => out.push(value),
        }
    }
    out.push('"');
    Ok(())
}

fn format_isolation(isolation: UiIsolation) -> String {
    format!("{isolation:?}")
}

fn format_blend_mode(blend_mode: UiBlendMode) -> String {
    format!("{blend_mode:?}")
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn push_comma_newline(out: &mut String, comma: bool) {
    if comma {
        out.push(',');
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{TakumiCompositingGroupId, TakumiEffectOutsets, TakumiPaintNodeId};
    use crate::metadata::ArcweftNodeMetadata;
    use arcweft_ui::{ContainerKind, FragmentKind, HandlerId, NodeId, NodeKey, StyleId};

    fn metadata() -> ArcweftNodeMetadata {
        ArcweftNodeMetadata::new(
            NodeId(42),
            NodeKey(420),
            FragmentKind::Container(ContainerKind::Stack),
            StyleId(7),
            [HandlerId(9)],
            None,
        )
    }

    #[test]
    fn evidence_json_contains_compositing_bounds_and_not_platform_identity() {
        let mut frame = TakumiCaptureFrame::default();
        frame.push_compositing_group(
            TakumiCompositingCaptureRecord::new(
                metadata(),
                TakumiCompositingGroupId::new(10),
                TakumiPaintNodeId::new(1),
                HitRect::new(10.0, 20.0, 100.0, 50.0),
                HitRect::new(10.0, 20.0, 100.0, 50.0),
            )
            .with_effect_outsets(TakumiEffectOutsets::new(12.0, 0.0, 0.0))
            .with_blend_mode(UiBlendMode::Multiply),
        );

        let json = capture_frame_to_json(&frame);
        assert!(json.contains(COMPOSITING_EVIDENCE_SCHEMA_VERSION));
        assert!(json.contains("\"visual_bounds\""));
        assert!(json.contains("\"effect_outsets\""));
        assert!(json.contains("\"blend_mode\": \"Multiply\""));
        for forbidden in ["HW", "NS", "web_"]
            .into_iter()
            .zip(["ND", "View", "sys"])
            .map(|(prefix, suffix)| format!("{prefix}{suffix}"))
        {
            assert!(!json.contains(&forbidden));
        }
    }
}
