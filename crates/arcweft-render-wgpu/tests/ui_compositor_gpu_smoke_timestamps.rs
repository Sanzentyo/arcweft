#[test]
#[ignore = "requires pinned native/web adapters and exact image-readback fixtures"]
fn gpu_smoke_clip_mask_and_motion_timestamps() {
    // Fixture contract for seq06.13 / seq06.13a:
    // 1. Build a retained UI style transition with timestamps 0 ms, 80 ms,
    //    160 ms, and 240 ms over background-color, opacity, scale, and
    //    outline-width.
    // 2. Lower each sampled style into the same UiScene subtree:
    //    - direct background range;
    //    - one group with inset clip, one group with ellipse clip, and one group
    //      with even-odd polygon clip;
    //    - one alpha mask and one luminance mask with differing mask-size,
    //      mask-position, and mask-repeat settings;
    //    - one CSS color-family blend mode fixture.
    // 3. Render through UiCompositor::render_scene into an offscreen texture on
    //    the shared wgpu path for both native and web.
    // 4. Compare perceptual hashes and per-channel drift against the thresholds
    //    documented in docs/implementation/seq-06.13-css-motion-effects-coverage-2026-07-03.md.
    //
    // Deterministic pass planning and motion samples are covered by non-ignored
    // tests. This exact-pixel test remains ignored until the repository's
    // pinned GPU golden harness is promoted for seq06.13 milestone validation.
}
