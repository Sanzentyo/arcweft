struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct UiCompositorUniform {
    matrix: mat4x4<f32>,
    offset: vec4<f32>,
    params0: vec4<f32>,
    params1: vec4<f32>,
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

fn mask_coverage(uv: vec2<f32>) -> f32 {
    let mask = textureSample(mask_texture, source_sampler, uv);
    if (uniform_data.params0.x > 0.5) {
        return dot(mask.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)) * mask.a;
    }
    return mask.a;
}

fn blend_channel_dodge(backdrop: f32, source: f32) -> f32 {
    if (source >= 1.0) {
        return 1.0;
    }
    return min(backdrop / (1.0 - source), 1.0);
}

fn blend_channel_burn(backdrop: f32, source: f32) -> f32 {
    if (source <= 0.0) {
        return 0.0;
    }
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
