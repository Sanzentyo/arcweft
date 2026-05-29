# WGSL shader

カスタム WGSL shader は ModuleItem として扱う。

## 種別

```rust
pub enum ShaderKind {
    Transition,
    PostProcess,
    SpriteMaterial,
    UiMaterial,
    VectorFill,
    TextEffect,
    ActivityRender,
    Compute,
}
```

## Shader DSL

```arcw
pub shader @shader.transition.dissolve: Transition
requires params.progress >= 0.0 && params.progress <= 1.0
capability {
    stage = fragment
    storage_buffer = false
    storage_texture = false
    compute = false
    max_uniform_bytes = 256
}
params {
    progress: f32 = 0.0
    edge_softness: f32 = 0.04
}
resources {
    from: texture_2d<f32>
    to: texture_2d<f32>
    noise: texture_2d<f32>
    samp: sampler
}
wgsl {
    struct Params {
        progress: f32,
        edge_softness: f32,
    }

    @group(0) @binding(0) var from_tex: texture_2d<f32>;
    @group(0) @binding(1) var to_tex: texture_2d<f32>;
    @group(0) @binding(2) var noise_tex: texture_2d<f32>;
    @group(0) @binding(3) var samp: sampler;
    @group(0) @binding(4) var<uniform> params: Params;

    @fragment
    fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
        let n = textureSample(noise_tex, samp, uv).r;
        let a = smoothstep(params.progress - params.edge_softness,
                           params.progress + params.edge_softness,
                           n);
        let c0 = textureSample(from_tex, samp, uv);
        let c1 = textureSample(to_tex, samp, uv);
        return mix(c0, c1, a);
    }
}
```

## 使用

```arcw
transition.goto(@flow.alice_intro, shader=@shader.transition.dissolve) {
    duration = 600ms
    params { edge_softness = 0.03 }
}
```

## Validation

precompile / hot reload 時に以下を行う。

```text
parse shader DSL
compose WGSL
source map生成
Naga validation
binding reflection
capability check
contract check
wgpu shader module / pipeline creation
preview render optional
```

## Hot reload

新 shader が成功するまで旧 pipeline を維持。

```text
new success → frame boundaryでswap
new failure → keep old + diagnostic
```

## Agent / CLI

```bash
arcw shader check game/shaders/*.arcw
arcw shader reflect shader.transition.dissolve --json
arcw shader preview shader.post.crt --out preview.png
arcw agent shader set-param shader.post.crt curvature 0.2
```


