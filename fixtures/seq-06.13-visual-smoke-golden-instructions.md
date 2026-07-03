# Visual Smoke and Golden Instructions

## Fixture objective

The deterministic unit tests verify planning and interpolation. The optional
visual fixture verifies that native and web execute the same compositor path for
motion, clip, mask, and blend at fixed timestamps.

## Scene recipe

Build a retained UI scene with:

1. Root solid background primitive.
2. A compositing group at `x=96, y=48, width=240, height=160`.
3. Two direct child rectangles with contrasting colors.
4. A transition on `BackgroundColor` from blue to red over `1000 ms`.
5. A transition on `Opacity` from `1000` to `500` over `1000 ms`.
6. A transition on `Scale` from `1000` to `1200` over `1000 ms`.
7. A polygon clip path with three vertices and `evenodd` fill rule.
8. One external mask texture with:
   - intrinsic size `32x16`;
   - `mask-size: 64px 32px`;
   - `mask-position: 50% 50%`;
   - `mask-repeat: no-repeat`;
   - one run using alpha channel and one run using luminance channel.
9. `mix-blend-mode: hue` in one run and `luminosity` in a second run.

## Timestamp samples

Capture these logical timestamps from the player timeline:

```text
0 ms
125 ms
250 ms
500 ms
750 ms
1000 ms
```

For each timestamp, record:

- `UiMotionSample` packets for background color, opacity, and scale;
- `UiCompositorPlan` pass counters;
- clip plan kind and local/visual bounds;
- mask tile origin, tile size, repeat flags, and channel;
- output image hash or bounded drift packet.

## Golden policy

Use exact image hashes only for pinned native/web adapter suites where:

- GPU adapter name and driver version are recorded;
- texture format is fixed;
- device pixel ratio is fixed;
- shader compiler backend is fixed.

For ordinary CI or developer machines, use drift packets:

```text
max_abs_channel_delta <= 2
mean_abs_channel_delta <= 0.25
alpha_coverage_pixel_count_delta <= 1% of group area
```

## Commands after overlay apply

```bash
cargo test -p arcweft-render-wgpu --test ui_compositor_gpu_smoke_timestamps --all-features -- --ignored --nocapture
```

The included ignored test is a fixture contract. It should be promoted once the
Arcweft GPU readback harness can save and compare native/web captures at the
same logical timestamps.
