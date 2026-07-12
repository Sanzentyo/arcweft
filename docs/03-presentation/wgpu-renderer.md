# wgpu renderer

## RenderSpec

```rust
pub struct RenderSpec {
    pub size: UVec2,
    pub clear: Color,
    pub layers: Vec<LayerSpec>,
    pub postprocess: Vec<ShaderPassSpec>,
}

pub enum LayerSpec {
    Sprite(SpriteSpec),
    Text(TextSpec),
    Vector(VectorSpec),
    View(ViewRenderSpec),
    Group(GroupSpec),
    Video(VideoSpec),
    CustomShader(CustomMaterialSpec),
}
```

## Render owner

GPU object は `RenderOwner` だけが作る。

```rust
pub enum GpuRequest {
    CreateTexture(TextureUpload),
    CreatePipeline(PipelineDesc),
    CreateShaderModule(ValidatedShader),
    UpdateBuffer(BufferUpdate),
}
```

worker は CPU 側準備だけ行い、GPU request を返す。

## Headless

headless は offscreen texture へ描画し、readback して PNG / raw RGBA / object-id / mask を生成する。

```rust
pub struct RenderCaptureOptions {
    pub size: UVec2,
    pub scale_factor: f32,
    pub color: bool,
    pub overlay: bool,
    pub object_id: bool,
    pub masks: MaskCaptureMode,
    pub include_view: bool,
}
```

## Object ID pass

通常 color pass とは別に object-id texture を作る。

```text
sprite alice       → object id 101
choice listen     → object id 205
dialogue view     → object id 300
```

これにより bbox / polygon / segmentation mask をエンジン情報から生成できる。
