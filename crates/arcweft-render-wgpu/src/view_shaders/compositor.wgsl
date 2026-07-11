struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct ViewCompositorUniform {
    matrix: mat4x4<f32>,
    offset: vec4<f32>,
    params0: vec4<f32>,
    params1: vec4<f32>,
    params2: vec4<f32>,
    clip_vertices: array<vec4<f32>, 96>,
    gradient_stops: array<vec4<f32>, 8>,
    pass_kind: u32,
    output_encoding: u32,
    seed_low: u32,
    seed_high: u32,
};

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var backdrop_texture: texture_2d<f32>;
@group(0) @binding(2) var mask_texture: texture_2d<f32>;
@group(0) @binding(3) var source_sampler: sampler;
@group(0) @binding(4) var<uniform> uniform_data: ViewCompositorUniform;

const PASS_COMPOSITE: u32 = 0u;
const PASS_COLOR_MATRIX: u32 = 1u;
const PASS_BLUR: u32 = 2u;
const PASS_DROP_SHADOW: u32 = 3u;
const PASS_MASK: u32 = 4u;
const PASS_BLEND: u32 = 5u;
const PASS_CLIP: u32 = 6u;
const PASS_BOX_SHADOW: u32 = 7u;
const PASS_MASK_GRADIENT: u32 = 8u;
const PASS_CLIPPED_COMPOSITE: u32 = 9u;
const PASS_TEXT_TINT: u32 = 10u;
const PASS_TEXT_DISPLACEMENT: u32 = 11u;
const PASS_TEXT_SPARKLE: u32 = 12u;
const OUTPUT_ENCODING_SRGB: u32 = 1u;
const PI: f32 = 3.141592653589793;
const TAU: f32 = 6.283185307179586;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    let position = positions[vertex_index];
    var out: VertexOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = position * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    return out;
}

fn source_color(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(source_texture, source_sampler, uv, 0.0);
}

fn backdrop_color(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(backdrop_texture, source_sampler, uv, 0.0);
}

fn apply_color_matrix(color: vec4<f32>) -> vec4<f32> {
    return clamp(uniform_data.matrix * color + uniform_data.offset, vec4<f32>(0.0), vec4<f32>(1.0));
}

fn clipped_rect_coverage(uv: vec2<f32>) -> f32 {
    let position = uv * uniform_data.params2.xy;
    let rect = uniform_data.params1;
    let right = rect.x + rect.z;
    let bottom = rect.y + rect.w;
    let inside = position.x >= rect.x && position.x <= right && position.y >= rect.y && position.y <= bottom;
    return select(0.0, 1.0, inside);
}

fn blur_color(uv: vec2<f32>) -> vec4<f32> {
    let step = uniform_data.params0.xy;
    let radius = max(uniform_data.params0.z, 0.0);
    let offset = step * max(radius, 1.0);
    var color = source_color(uv) * 0.2270270270;
    color = color + source_color(uv + offset * 1.3846153846) * 0.3162162162;
    color = color + source_color(uv - offset * 1.3846153846) * 0.3162162162;
    color = color + source_color(uv + offset * 3.2307692308) * 0.0702702703;
    color = color + source_color(uv - offset * 3.2307692308) * 0.0702702703;
    return color;
}

fn hash_u32(value_in: u32) -> f32 {
    var value = value_in;
    value = value ^ (value >> 16u);
    value = value * 0x7feb352du;
    value = value ^ (value >> 15u);
    value = value * 0x846ca68bu;
    value = value ^ (value >> 16u);
    return f32(value & 0x00ffffffu) / 16777215.0;
}

fn text_tint_color(source: vec4<f32>) -> vec4<f32> {
    if (source.a <= 0.0 || all(source.rgb == vec3<f32>(0.0))) { return source; }
    let amount = clamp(uniform_data.params1.x, 0.0, 1.0);
    return vec4<f32>(mix(source.rgb, uniform_data.params0.rgb, vec3<f32>(amount)), source.a);
}

fn text_displacement_color(uv: vec2<f32>) -> vec4<f32> {
    let extent = max(uniform_data.params2.xy, vec2<f32>(1.0));
    let position = uv * extent;
    let direction = uniform_data.params1.xy;
    let axis = select(position.x, position.y, abs(direction.x) >= abs(direction.y));
    let amplitude = uniform_data.params0.x;
    let period = max(uniform_data.params0.y, 0.0001);
    let phase = uniform_data.params0.z;
    let displacement_kind = u32(uniform_data.params0.w);
    var delta = sin(axis / period * TAU + phase) * amplitude;
    if (displacement_kind != 0u) {
        let time_key = select(u32(abs(phase) * 1000.0), 0u, displacement_kind == 2u);
        let row_key = u32(max(round(axis), 0.0));
        let noise = hash_u32(
            row_key ^ uniform_data.seed_low ^ (uniform_data.seed_high * 0x9e3779b9u) ^ time_key,
        );
        delta = (noise * 2.0 - 1.0) * amplitude;
    }
    let sample_uv = clamp(uv - direction * delta / extent, vec2<f32>(0.0), vec2<f32>(1.0));
    return source_color(sample_uv);
}

fn text_sparkle_color(uv: vec2<f32>, source: vec4<f32>) -> vec4<f32> {
    if (source.a <= 0.0 || all(source.rgb == vec3<f32>(0.0))) { return source; }
    let extent = max(uniform_data.params2.xy, vec2<f32>(1.0));
    let pixel = vec2<u32>(max(floor(uv * extent), vec2<f32>(0.0)));
    let phase = uniform_data.params0.y;
    let phase_key = u32(abs(phase) * 1000.0);
    let noise = hash_u32(
        pixel.x * 0x9e3779b9u ^ pixel.y * 0x85ebca6bu ^ uniform_data.seed_low
            ^ uniform_data.seed_high ^ phase_key,
    );
    let seed_phase = f32(uniform_data.seed_low & 0xffu) * 0.001;
    let shimmer = sin(phase + seed_phase * TAU) * 0.5 + 0.5;
    let amount = clamp(uniform_data.params0.x, 0.0, 1.0);
    let pulse = clamp((noise + shimmer) * amount, 0.0, 1.0);
    let red = mix(source.r, 1.0, pulse * 0.45);
    let green = mix(source.g, 225.0 / 255.0, pulse * 0.35);
    let blue = mix(source.b, 1.0, pulse * 0.55);
    return vec4<f32>(red, green, blue, source.a);
}

fn shadow_color(uv: vec2<f32>) -> vec4<f32> {
    let alpha = textureSampleLevel(source_texture, source_sampler, uv - uniform_data.params0.xy, 0.0).a;
    return vec4<f32>(uniform_data.params1.rgb * uniform_data.params1.a, alpha * uniform_data.params1.a);
}

fn mask_axis_uv(position_px: f32, origin_px: f32, tile_size_px_in: f32, stride_px_in: f32, tile_count_f: f32, mode_f: f32) -> f32 {
    let tile_size_px = max(tile_size_px_in, 0.0001);
    let stride_px = max(stride_px_in, 0.0001);
    let mode = u32(mode_f + 0.5);
    let local = position_px - origin_px;
    if (mode == 0u) {
        let raw = local / tile_size_px;
        if (raw < 0.0 || raw > 1.0) { return -1.0; }
        return clamp(raw, 0.0, 1.0);
    }
    if (mode == 1u) {
        return fract(local / tile_size_px);
    }

    let count = u32(max(tile_count_f, 0.0) + 0.5);
    if (count == 0u) { return -1.0; }
    let index = floor(local / stride_px + 0.00001);
    if (index < 0.0 || index >= f32(count)) { return -1.0; }
    let offset = local - index * stride_px;
    if (offset < 0.0 || offset > tile_size_px) { return -1.0; }
    return clamp(offset / tile_size_px, 0.0, 1.0);
}

fn mask_tile_uv(uv: vec2<f32>) -> vec2<f32> {
    let source_position_px = uv * uniform_data.params2.xy;
    return vec2<f32>(
        mask_axis_uv(
            source_position_px.x,
            uniform_data.params1.x,
            uniform_data.params1.z,
            uniform_data.params2.z,
            uniform_data.offset.x,
            uniform_data.params0.y,
        ),
        mask_axis_uv(
            source_position_px.y,
            uniform_data.params1.y,
            uniform_data.params1.w,
            uniform_data.params2.w,
            uniform_data.offset.y,
            uniform_data.params0.z,
        ),
    );
}

fn texture_mask_coverage(uv: vec2<f32>) -> f32 {
    let mask_uv = mask_tile_uv(uv);
    if (mask_uv.x < 0.0 || mask_uv.y < 0.0) { return 0.0; }
    let mask = textureSampleLevel(mask_texture, source_sampler, mask_uv, 0.0);
    if (uniform_data.params0.x > 0.5) {
        return dot(mask.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)) * mask.a;
    }
    return mask.a;
}

fn stop_coverage(stop: vec4<f32>) -> f32 {
    return select(stop.y, stop.z, uniform_data.params0.x > 0.5);
}

fn gradient_stop_coverage(t_in: f32) -> f32 {
    let t = clamp(t_in, 0.0, 1.0);
    let count = u32(max(uniform_data.matrix[1].z, 0.0) + 0.5);
    if (count == 0u) { return 0.0; }
    var previous = uniform_data.gradient_stops[0];
    if (t <= previous.x || count == 1u) { return stop_coverage(previous); }
    for (var i = 1u; i < 8u; i = i + 1u) {
        if (i >= count) { break; }
        let current = uniform_data.gradient_stops[i];
        if (t <= current.x) {
            let span = max(current.x - previous.x, 0.0001);
            let mix_t = clamp((t - previous.x) / span, 0.0, 1.0);
            return mix(stop_coverage(previous), stop_coverage(current), mix_t);
        }
        previous = current;
    }
    return stop_coverage(previous);
}

fn generated_gradient_coverage(uv: vec2<f32>) -> f32 {
    let tile_uv = mask_tile_uv(uv);
    if (tile_uv.x < 0.0 || tile_uv.y < 0.0) { return 0.0; }
    let kind = u32(uniform_data.params0.w + 0.5);
    if (kind == 1u) {
        let angle = uniform_data.matrix[0].x * PI / 180.0;
        let axis = vec2<f32>(cos(angle), sin(angle));
        let t = dot(tile_uv - vec2<f32>(0.5), axis) + 0.5;
        return gradient_stop_coverage(t);
    }
    let tile_px = tile_uv * uniform_data.params1.zw;
    if (kind == 2u) {
        let center = uniform_data.matrix[0].yz;
        let radius = max(uniform_data.matrix[1].xy, vec2<f32>(0.0001));
        return gradient_stop_coverage(length((tile_px - center) / radius));
    }
    if (kind == 3u) {
        let center = uniform_data.matrix[0].yz;
        let start_turns = uniform_data.matrix[0].w / 360.0;
        let delta = tile_px - center;
        let t = fract(atan2(delta.y, delta.x) / TAU - start_turns + 1.0);
        return gradient_stop_coverage(t);
    }
    return 1.0;
}

fn logical_position(uv: vec2<f32>) -> vec2<f32> {
    return uniform_data.params1.xy + uv * uniform_data.params2.xy;
}

fn inset_clip_coverage(position: vec2<f32>) -> f32 {
    let rect = uniform_data.matrix[0];
    let radii = max(uniform_data.matrix[1], vec4<f32>(0.0));
    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.z;
    let bottom = rect.y + rect.w;
    if (position.x < left || position.x > right || position.y < top || position.y > bottom) {
        return 0.0;
    }
    var radius = 0.0;
    var center = position;
    if (position.x < left + radii.x && position.y < top + radii.x) {
        radius = radii.x;
        center = vec2<f32>(left + radius, top + radius);
    } else if (position.x > right - radii.y && position.y < top + radii.y) {
        radius = radii.y;
        center = vec2<f32>(right - radius, top + radius);
    } else if (position.x > right - radii.z && position.y > bottom - radii.z) {
        radius = radii.z;
        center = vec2<f32>(right - radius, bottom - radius);
    } else if (position.x < left + radii.w && position.y > bottom - radii.w) {
        radius = radii.w;
        center = vec2<f32>(left + radius, bottom - radius);
    }
    if (radius > 0.0 && distance(position, center) > radius) { return 0.0; }
    return 1.0;
}

fn ellipse_clip_coverage(position: vec2<f32>) -> f32 {
    let ellipse = uniform_data.matrix[0];
    let radius = max(ellipse.zw, vec2<f32>(0.0001));
    let normalized = (position - ellipse.xy) / radius;
    return select(0.0, 1.0, dot(normalized, normalized) <= 1.0);
}

fn is_left(a: vec2<f32>, b: vec2<f32>, p: vec2<f32>) -> f32 {
    return (b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y);
}

fn polygon_even_odd_coverage(position: vec2<f32>, count: u32) -> f32 {
    var inside = false;
    var previous = uniform_data.clip_vertices[count - 1u].xy;
    for (var i = 0u; i < 16u; i = i + 1u) {
        if (i >= count) { break; }
        let current = uniform_data.clip_vertices[i].xy;
        if (((current.y > position.y) != (previous.y > position.y)) &&
            (position.x < (previous.x - current.x) * (position.y - current.y) / (previous.y - current.y) + current.x)) {
            inside = !inside;
        }
        previous = current;
    }
    return select(0.0, 1.0, inside);
}

fn polygon_non_zero_coverage(position: vec2<f32>, count: u32) -> f32 {
    var winding = 0i;
    var previous = uniform_data.clip_vertices[count - 1u].xy;
    for (var i = 0u; i < 16u; i = i + 1u) {
        if (i >= count) { break; }
        let current = uniform_data.clip_vertices[i].xy;
        if (previous.y <= position.y) {
            if (current.y > position.y && is_left(previous, current, position) > 0.0) { winding = winding + 1i; }
        } else if (current.y <= position.y && is_left(previous, current, position) < 0.0) {
            winding = winding - 1i;
        }
        previous = current;
    }
    return select(0.0, 1.0, winding != 0i);
}

fn path_even_odd_coverage(position: vec2<f32>, count: u32) -> f32 {
    var inside = false;
    for (var i = 0u; i < 96u; i = i + 1u) {
        if (i >= count) { break; }
        let edge = uniform_data.clip_vertices[i];
        let a = edge.xy;
        let b = edge.zw;
        if (((a.y > position.y) != (b.y > position.y)) &&
            (position.x < (b.x - a.x) * (position.y - a.y) / (b.y - a.y) + a.x)) {
            inside = !inside;
        }
    }
    return select(0.0, 1.0, inside);
}

fn path_non_zero_coverage(position: vec2<f32>, count: u32) -> f32 {
    var winding = 0i;
    for (var i = 0u; i < 96u; i = i + 1u) {
        if (i >= count) { break; }
        let edge = uniform_data.clip_vertices[i];
        let a = edge.xy;
        let b = edge.zw;
        if (a.y <= position.y) {
            if (b.y > position.y && is_left(a, b, position) > 0.0) { winding = winding + 1i; }
        } else if (b.y <= position.y && is_left(a, b, position) < 0.0) {
            winding = winding - 1i;
        }
    }
    return select(0.0, 1.0, winding != 0i);
}

fn clip_coverage(uv: vec2<f32>) -> f32 {
    let kind = u32(uniform_data.params0.x + 0.5);
    let position = logical_position(uv);
    if (kind == 1u) { return inset_clip_coverage(position); }
    if (kind == 2u) { return ellipse_clip_coverage(position); }
    if (kind == 3u) {
        let count = u32(uniform_data.params0.z + 0.5);
        if (count < 3u) { return 0.0; }
        if (uniform_data.params0.y > 0.5) { return polygon_even_odd_coverage(position, count); }
        return polygon_non_zero_coverage(position, count);
    }
    if (kind == 4u) {
        let count = u32(uniform_data.params0.z + 0.5);
        if (count == 0u) { return 0.0; }
        if (uniform_data.params0.y > 0.5) { return path_even_odd_coverage(position, count); }
        return path_non_zero_coverage(position, count);
    }
    return 1.0;
}

fn rounded_rect_coverage_at(
    position: vec2<f32>,
    rect: vec4<f32>,
    radii0_in: vec4<f32>,
    radii1_in: vec4<f32>,
) -> f32 {
    let radii0 = max(radii0_in, vec4<f32>(0.0));
    let radii1 = max(radii1_in, vec4<f32>(0.0));
    let top_left = radii0.xy;
    let top_right = radii0.zw;
    let bottom_right = radii1.xy;
    let bottom_left = radii1.zw;
    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.z;
    let bottom = rect.y + rect.w;
    if (position.x < left || position.x > right || position.y < top || position.y > bottom) {
        return 0.0;
    }

    var radius = vec2<f32>(0.0);
    var center = position;
    if (position.x < left + top_left.x && position.y < top + top_left.y) {
        radius = top_left;
        center = vec2<f32>(left + radius.x, top + radius.y);
    } else if (position.x > right - top_right.x && position.y < top + top_right.y) {
        radius = top_right;
        center = vec2<f32>(right - radius.x, top + radius.y);
    } else if (position.x > right - bottom_right.x && position.y > bottom - bottom_right.y) {
        radius = bottom_right;
        center = vec2<f32>(right - radius.x, bottom - radius.y);
    } else if (position.x < left + bottom_left.x && position.y > bottom - bottom_left.y) {
        radius = bottom_left;
        center = vec2<f32>(left + radius.x, bottom - radius.y);
    }

    if (radius.x > 0.0001 && radius.y > 0.0001) {
        let normalized = (position - center) / radius;
        if (dot(normalized, normalized) > 1.0) { return 0.0; }
    }
    return 1.0;
}

fn rounded_rect_signed_distance(
    position: vec2<f32>,
    rect: vec4<f32>,
    radii0_in: vec4<f32>,
    radii1_in: vec4<f32>,
) -> f32 {
    let radii0 = max(radii0_in, vec4<f32>(0.0));
    let radii1 = max(radii1_in, vec4<f32>(0.0));
    let safe_size = max(rect.zw, vec2<f32>(0.0001));
    let center = rect.xy + safe_size * 0.5;
    var radius = vec2<f32>(0.0);
    if (position.x <= center.x && position.y <= center.y) {
        radius = radii0.xy;
    } else if (position.x > center.x && position.y <= center.y) {
        radius = radii0.zw;
    } else if (position.x > center.x && position.y > center.y) {
        radius = radii1.xy;
    } else {
        radius = radii1.zw;
    }

    let clamped_radius = min(radius, safe_size * 0.5);
    let half_size = safe_size * 0.5;
    let inner_half = max(half_size - clamped_radius, vec2<f32>(0.0001));
    let q = abs(position - center) - inner_half;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - min(clamped_radius.x, clamped_radius.y);
}

fn box_shadow_caster_coverage(
    position: vec2<f32>,
    rect: vec4<f32>,
    radii0: vec4<f32>,
    radii1: vec4<f32>,
    blur: f32,
) -> f32 {
    if (blur <= 0.0001) { return rounded_rect_coverage_at(position, rect, radii0, radii1); }
    let distance = rounded_rect_signed_distance(position, rect, radii0, radii1);
    let softness = max(blur * 1.5, 1.0);
    return 1.0 - smoothstep(-softness, softness, distance);
}

fn box_shadow_color(uv: vec2<f32>) -> vec4<f32> {
    let position = logical_position(uv);
    let body_rect = uniform_data.matrix[0];
    let shadow_rect = uniform_data.matrix[1];
    let body_radii0 = uniform_data.matrix[2];
    let body_radii1 = uniform_data.matrix[3];
    let shadow_radii0 = uniform_data.clip_vertices[0];
    let shadow_radii1 = uniform_data.clip_vertices[1];
    let blur = max(uniform_data.params0.x, 0.0);
    let kind = u32(uniform_data.params0.w + 0.5);
    let caster = box_shadow_caster_coverage(position, shadow_rect, shadow_radii0, shadow_radii1, blur);
    let body = rounded_rect_coverage_at(position, body_rect, body_radii0, body_radii1);
    let outer_coverage = caster * (1.0 - body);
    let inset_coverage = body * (1.0 - caster);
    let coverage = select(outer_coverage, inset_coverage, kind == 1u);
    return vec4<f32>(uniform_data.offset.rgb, uniform_data.offset.a * coverage);
}

fn blend_channel_dodge(backdrop: f32, source: f32) -> f32 {
    if (source >= 1.0) { return 1.0; }
    return min(backdrop / (1.0 - source), 1.0);
}

fn blend_channel_burn(backdrop: f32, source: f32) -> f32 {
    if (source <= 0.0) { return 0.0; }
    return 1.0 - min((1.0 - backdrop) / source, 1.0);
}

fn soft_light(backdrop: f32, source: f32) -> f32 {
    let d = select(((16.0 * backdrop - 12.0) * backdrop + 4.0) * backdrop, sqrt(backdrop), backdrop > 0.25);
    return select(
        backdrop - (1.0 - 2.0 * source) * backdrop * (1.0 - backdrop),
        backdrop + (2.0 * source - 1.0) * (d - backdrop),
        source > 0.5,
    );
}

fn rgb_to_hsl(color: vec3<f32>) -> vec3<f32> {
    let max_channel = max(max(color.r, color.g), color.b);
    let min_channel = min(min(color.r, color.g), color.b);
    let delta = max_channel - min_channel;
    let lightness = (max_channel + min_channel) * 0.5;
    if (delta <= 0.00001) { return vec3<f32>(0.0, 0.0, lightness); }
    let saturation = delta / (1.0 - abs(2.0 * lightness - 1.0));
    var hue = 0.0;
    if (max_channel == color.r) {
        hue = (color.g - color.b) / delta;
    } else if (max_channel == color.g) {
        hue = (color.b - color.r) / delta + 2.0;
    } else {
        hue = (color.r - color.g) / delta + 4.0;
    }
    hue = fract(hue / 6.0 + 1.0);
    return vec3<f32>(hue, saturation, lightness);
}

fn hue_to_rgb(p: f32, q: f32, t_in: f32) -> f32 {
    var t = fract(t_in + 1.0);
    if (t < 1.0 / 6.0) { return p + (q - p) * 6.0 * t; }
    if (t < 1.0 / 2.0) { return q; }
    if (t < 2.0 / 3.0) { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    return p;
}

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let hue = hsl.x;
    let saturation = clamp(hsl.y, 0.0, 1.0);
    let lightness = clamp(hsl.z, 0.0, 1.0);
    if (saturation <= 0.00001) { return vec3<f32>(lightness); }
    let q = select(lightness + saturation - lightness * saturation, lightness * (1.0 + saturation), lightness < 0.5);
    let p = 2.0 * lightness - q;
    return vec3<f32>(
        hue_to_rgb(p, q, hue + 1.0 / 3.0),
        hue_to_rgb(p, q, hue),
        hue_to_rgb(p, q, hue - 1.0 / 3.0),
    );
}

fn blend_hsl_family(mode: u32, backdrop: vec3<f32>, source: vec3<f32>) -> vec3<f32> {
    let backdrop_hsl = rgb_to_hsl(backdrop);
    let source_hsl = rgb_to_hsl(source);
    if (mode == 14u) { return hsl_to_rgb(vec3<f32>(source_hsl.x, backdrop_hsl.y, backdrop_hsl.z)); }
    if (mode == 15u) { return hsl_to_rgb(vec3<f32>(backdrop_hsl.x, source_hsl.y, backdrop_hsl.z)); }
    if (mode == 16u) { return hsl_to_rgb(vec3<f32>(source_hsl.x, source_hsl.y, backdrop_hsl.z)); }
    if (mode == 17u) { return hsl_to_rgb(vec3<f32>(backdrop_hsl.x, backdrop_hsl.y, source_hsl.z)); }
    return source;
}

fn blend_rgb(mode: u32, backdrop: vec3<f32>, source: vec3<f32>) -> vec3<f32> {
    if (mode == 1u) { return backdrop * source; }
    if (mode == 2u) { return backdrop + source - backdrop * source; }
    if (mode == 3u) {
        return vec3<f32>(
            select(2.0 * backdrop.r * source.r, 1.0 - 2.0 * (1.0 - backdrop.r) * (1.0 - source.r), backdrop.r > 0.5),
            select(2.0 * backdrop.g * source.g, 1.0 - 2.0 * (1.0 - backdrop.g) * (1.0 - source.g), backdrop.g > 0.5),
            select(2.0 * backdrop.b * source.b, 1.0 - 2.0 * (1.0 - backdrop.b) * (1.0 - source.b), backdrop.b > 0.5),
        );
    }
    if (mode == 4u) { return min(backdrop, source); }
    if (mode == 5u) { return max(backdrop, source); }
    if (mode == 6u) { return vec3<f32>(blend_channel_dodge(backdrop.r, source.r), blend_channel_dodge(backdrop.g, source.g), blend_channel_dodge(backdrop.b, source.b)); }
    if (mode == 7u) { return vec3<f32>(blend_channel_burn(backdrop.r, source.r), blend_channel_burn(backdrop.g, source.g), blend_channel_burn(backdrop.b, source.b)); }
    if (mode == 8u) {
        return vec3<f32>(
            select(2.0 * backdrop.r * source.r, 1.0 - 2.0 * (1.0 - backdrop.r) * (1.0 - source.r), source.r > 0.5),
            select(2.0 * backdrop.g * source.g, 1.0 - 2.0 * (1.0 - backdrop.g) * (1.0 - source.g), source.g > 0.5),
            select(2.0 * backdrop.b * source.b, 1.0 - 2.0 * (1.0 - backdrop.b) * (1.0 - source.b), source.b > 0.5),
        );
    }
    if (mode == 9u) { return vec3<f32>(soft_light(backdrop.r, source.r), soft_light(backdrop.g, source.g), soft_light(backdrop.b, source.b)); }
    if (mode == 10u) { return abs(backdrop - source); }
    if (mode == 11u) { return backdrop + source - 2.0 * backdrop * source; }
    if (mode == 12u) { return min(backdrop + source, vec3<f32>(1.0)); }
    if (mode == 13u) { return max(backdrop + source - vec3<f32>(1.0), vec3<f32>(0.0)); }
    if (mode >= 14u && mode <= 17u) { return blend_hsl_family(mode, backdrop, source); }
    return source;
}

fn composite_source_over(backdrop: vec4<f32>, source: vec4<f32>) -> vec4<f32> {
    let out_alpha = source.a + backdrop.a * (1.0 - source.a);
    if (out_alpha <= 0.0) { return vec4<f32>(0.0); }
    let out_rgb = (source.rgb * source.a + backdrop.rgb * backdrop.a * (1.0 - source.a)) / out_alpha;
    return vec4<f32>(out_rgb, out_alpha);
}

fn srgb_encode_channel(value_in: f32) -> f32 {
    let value = clamp(value_in, 0.0, 1.0);
    if (value <= 0.0031308) {
        return value * 12.92;
    }
    return 1.055 * pow(value, 1.0 / 2.4) - 0.055;
}

fn srgb_encode(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_encode_channel(color.r),
        srgb_encode_channel(color.g),
        srgb_encode_channel(color.b),
    );
}

fn encode_output(color: vec4<f32>) -> vec4<f32> {
    if (uniform_data.output_encoding == OUTPUT_ENCODING_SRGB) {
        return vec4<f32>(srgb_encode(color.rgb), color.a);
    }
    return color;
}

fn fragment_color(in: VertexOut) -> vec4<f32> {
    let source = source_color(in.uv);
    if (uniform_data.pass_kind == PASS_COLOR_MATRIX) { return apply_color_matrix(source); }
    if (uniform_data.pass_kind == PASS_BLUR) { return blur_color(in.uv); }
    if (uniform_data.pass_kind == PASS_DROP_SHADOW) {
        let shadow = shadow_color(in.uv);
        return composite_source_over(shadow, source);
    }
    if (uniform_data.pass_kind == PASS_MASK) {
        let coverage = texture_mask_coverage(in.uv);
        return vec4<f32>(source.rgb, source.a * coverage);
    }
    if (uniform_data.pass_kind == PASS_MASK_GRADIENT) {
        let coverage = generated_gradient_coverage(in.uv);
        return vec4<f32>(source.rgb, source.a * coverage);
    }
    if (uniform_data.pass_kind == PASS_CLIP) {
        let coverage = clip_coverage(in.uv);
        return vec4<f32>(source.rgb, source.a * coverage);
    }
    if (uniform_data.pass_kind == PASS_BOX_SHADOW) { return box_shadow_color(in.uv); }
    if (uniform_data.pass_kind == PASS_CLIPPED_COMPOSITE) {
        let coverage = clipped_rect_coverage(in.uv);
        let opacity = clamp(uniform_data.params0.x, 0.0, 1.0);
        return vec4<f32>(source.rgb, source.a * opacity * coverage);
    }
    if (uniform_data.pass_kind == PASS_TEXT_TINT) { return text_tint_color(source); }
    if (uniform_data.pass_kind == PASS_TEXT_DISPLACEMENT) {
        return text_displacement_color(in.uv);
    }
    if (uniform_data.pass_kind == PASS_TEXT_SPARKLE) {
        return text_sparkle_color(in.uv, source);
    }
    if (uniform_data.pass_kind == PASS_BLEND) {
        let backdrop = backdrop_color(in.uv);
        let mode = u32(uniform_data.params0.y);
        let opacity = clamp(uniform_data.params0.x, 0.0, 1.0);
        let adjusted_source = vec4<f32>(source.rgb, source.a * opacity);
        let blended_rgb = blend_rgb(mode, backdrop.rgb, adjusted_source.rgb);
        let blended = vec4<f32>(blended_rgb, adjusted_source.a);
        return composite_source_over(backdrop, blended);
    }
    return vec4<f32>(source.rgb, source.a * clamp(uniform_data.params0.x, 0.0, 1.0));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return encode_output(fragment_color(in));
}
