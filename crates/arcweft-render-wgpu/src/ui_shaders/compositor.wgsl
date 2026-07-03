struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct UiCompositorUniform {
    matrix: mat4x4<f32>,
    offset: vec4<f32>,
    params0: vec4<f32>,
    params1: vec4<f32>,
    params2: vec4<f32>,
    clip_vertices: array<vec4<f32>, 16>,
    pass_kind: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
};

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var backdrop_texture: texture_2d<f32>;
@group(0) @binding(2) var mask_texture: texture_2d<f32>;
@group(0) @binding(3) var source_sampler: sampler;
@group(0) @binding(4) var<uniform> uniform_data: UiCompositorUniform;

const PASS_COMPOSITE: u32 = 0u;
const PASS_COLOR_MATRIX: u32 = 1u;
const PASS_BLUR: u32 = 2u;
const PASS_DROP_SHADOW: u32 = 3u;
const PASS_MASK: u32 = 4u;
const PASS_BLEND: u32 = 5u;
const PASS_CLIP: u32 = 6u;

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
    return textureSample(source_texture, source_sampler, uv);
}

fn backdrop_color(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(backdrop_texture, source_sampler, uv);
}

fn apply_color_matrix(color: vec4<f32>) -> vec4<f32> {
    return clamp(uniform_data.matrix * color + uniform_data.offset, vec4<f32>(0.0), vec4<f32>(1.0));
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

fn shadow_color(uv: vec2<f32>) -> vec4<f32> {
    let alpha = textureSample(source_texture, source_sampler, uv - uniform_data.params0.xy).a;
    return vec4<f32>(uniform_data.params1.rgb * uniform_data.params1.a, alpha * uniform_data.params1.a);
}

fn mask_axis_uv(position_px: f32, tile_size_px: f32, repeat_enabled: f32) -> f32 {
    if (tile_size_px <= 0.0) {
        return -1.0;
    }
    let raw = position_px / tile_size_px;
    if (repeat_enabled > 0.5) {
        return fract(raw);
    }
    if (raw < 0.0 || raw > 1.0) {
        return -1.0;
    }
    return clamp(raw, 0.0, 1.0);
}

fn mask_coverage(uv: vec2<f32>) -> f32 {
    let source_position_px = uv * uniform_data.params2.xy;
    let mask_position_px = source_position_px - uniform_data.params1.xy;
    let mask_uv = vec2<f32>(
        mask_axis_uv(mask_position_px.x, uniform_data.params1.z, uniform_data.params0.y),
        mask_axis_uv(mask_position_px.y, uniform_data.params1.w, uniform_data.params0.z),
    );
    if (mask_uv.x < 0.0 || mask_uv.y < 0.0) {
        return 0.0;
    }
    let mask = textureSample(mask_texture, source_sampler, mask_uv);
    if (uniform_data.params0.x > 0.5) {
        return dot(mask.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)) * mask.a;
    }
    return mask.a;
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

    if (radius > 0.0 && distance(position, center) > radius) {
        return 0.0;
    }
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
            if (current.y > position.y && is_left(previous, current, position) > 0.0) {
                winding = winding + 1i;
            }
        } else if (current.y <= position.y && is_left(previous, current, position) < 0.0) {
            winding = winding - 1i;
        }
        previous = current;
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
        if (uniform_data.params0.y > 0.5) {
            return polygon_even_odd_coverage(position, count);
        }
        return polygon_non_zero_coverage(position, count);
    }
    return 1.0;
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
    if (delta <= 0.00001) {
        return vec3<f32>(0.0, 0.0, lightness);
    }
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
    if (saturation <= 0.00001) {
        return vec3<f32>(lightness);
    }
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
    if (mode == 6u) {
        return vec3<f32>(
            blend_channel_dodge(backdrop.r, source.r),
            blend_channel_dodge(backdrop.g, source.g),
            blend_channel_dodge(backdrop.b, source.b),
        );
    }
    if (mode == 7u) {
        return vec3<f32>(
            blend_channel_burn(backdrop.r, source.r),
            blend_channel_burn(backdrop.g, source.g),
            blend_channel_burn(backdrop.b, source.b),
        );
    }
    if (mode == 8u) {
        return vec3<f32>(
            select(2.0 * backdrop.r * source.r, 1.0 - 2.0 * (1.0 - backdrop.r) * (1.0 - source.r), source.r > 0.5),
            select(2.0 * backdrop.g * source.g, 1.0 - 2.0 * (1.0 - backdrop.g) * (1.0 - source.g), source.g > 0.5),
            select(2.0 * backdrop.b * source.b, 1.0 - 2.0 * (1.0 - backdrop.b) * (1.0 - source.b), source.b > 0.5),
        );
    }
    if (mode == 9u) {
        return vec3<f32>(
            soft_light(backdrop.r, source.r),
            soft_light(backdrop.g, source.g),
            soft_light(backdrop.b, source.b),
        );
    }
    if (mode == 10u) { return abs(backdrop - source); }
    if (mode == 11u) { return backdrop + source - 2.0 * backdrop * source; }
    if (mode == 12u) { return min(backdrop + source, vec3<f32>(1.0)); }
    if (mode == 13u) { return max(backdrop + source - vec3<f32>(1.0), vec3<f32>(0.0)); }
    if (mode >= 14u && mode <= 17u) { return blend_hsl_family(mode, backdrop, source); }
    return source;
}

fn composite_source_over(backdrop: vec4<f32>, source: vec4<f32>) -> vec4<f32> {
    let out_alpha = source.a + backdrop.a * (1.0 - source.a);
    if (out_alpha <= 0.0) {
        return vec4<f32>(0.0);
    }
    let out_rgb = (source.rgb * source.a + backdrop.rgb * backdrop.a * (1.0 - source.a)) / out_alpha;
    return vec4<f32>(out_rgb, out_alpha);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let source = source_color(in.uv);
    if (uniform_data.pass_kind == PASS_COLOR_MATRIX) {
        return apply_color_matrix(source);
    }
    if (uniform_data.pass_kind == PASS_BLUR) {
        return blur_color(in.uv);
    }
    if (uniform_data.pass_kind == PASS_DROP_SHADOW) {
        let shadow = shadow_color(in.uv);
        return composite_source_over(shadow, source);
    }
    if (uniform_data.pass_kind == PASS_MASK) {
        let coverage = mask_coverage(in.uv);
        return vec4<f32>(source.rgb, source.a * coverage);
    }
    if (uniform_data.pass_kind == PASS_CLIP) {
        let coverage = clip_coverage(in.uv);
        return vec4<f32>(source.rgb, source.a * coverage);
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
