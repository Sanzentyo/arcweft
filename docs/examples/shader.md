# Shader example

```awft
pub shader #shader.post.crt: PostProcess
capability { stage = fragment; storage_buffer = false; compute = false }
params {
    curvature: f32 = 0.12
    vignette: f32 = 0.35
}
resources {
    source: texture_2d<f32>
    samp: sampler
}
wgsl {
    struct Params { curvature: f32, vignette: f32 }
    @group(0) @binding(0) var source_tex: texture_2d<f32>;
    @group(0) @binding(1) var samp: sampler;
    @group(0) @binding(2) var<uniform> params: Params;

    @fragment
    fn fs_main(@location(0) uv_in: vec2<f32>) -> @location(0) vec4<f32> {
        let centered = uv_in * 2.0 - vec2<f32>(1.0, 1.0);
        let uv = centered * 0.5 + vec2<f32>(0.5, 0.5);
        return textureSample(source_tex, samp, uv);
    }
}
```

