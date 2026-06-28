use num_traits::ToPrimitive;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UiProgramRevision(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewFragmentRevision(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StyleRevision(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextLayoutRevision(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImageRevision(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RendererResourceRevision(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewportKey {
    width_px: u32,
    height_px: u32,
    device_pixel_ratio_milli: u32,
    font_px_milli: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TakumiSceneCacheKey {
    program: UiProgramRevision,
    fragment: ViewFragmentRevision,
    style: StyleRevision,
    text: TextLayoutRevision,
    images: ImageRevision,
    viewport: ViewportKey,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TakumiPaintCacheKey {
    scene: TakumiSceneCacheKey,
    renderer_resources: RendererResourceRevision,
}

impl ViewportKey {
    pub fn new(width_px: u32, height_px: u32, device_pixel_ratio: f32, font_px: f32) -> Self {
        Self {
            width_px,
            height_px,
            device_pixel_ratio_milli: milli_key(device_pixel_ratio),
            font_px_milli: milli_key(font_px),
        }
    }

    pub const fn width_px(self) -> u32 {
        self.width_px
    }

    pub const fn height_px(self) -> u32 {
        self.height_px
    }

    pub const fn device_pixel_ratio_milli(self) -> u32 {
        self.device_pixel_ratio_milli
    }

    pub const fn font_px_milli(self) -> u32 {
        self.font_px_milli
    }
}

impl TakumiSceneCacheKey {
    pub const fn new(
        program: UiProgramRevision,
        fragment: ViewFragmentRevision,
        style: StyleRevision,
        text: TextLayoutRevision,
        images: ImageRevision,
        viewport: ViewportKey,
    ) -> Self {
        Self {
            program,
            fragment,
            style,
            text,
            images,
            viewport,
        }
    }

    pub const fn paint_only_key(
        self,
        renderer_resources: RendererResourceRevision,
    ) -> TakumiPaintCacheKey {
        TakumiPaintCacheKey {
            scene: self,
            renderer_resources,
        }
    }

    pub const fn style_revision(self) -> StyleRevision {
        self.style
    }

    pub const fn text_revision(self) -> TextLayoutRevision {
        self.text
    }

    pub const fn image_revision(self) -> ImageRevision {
        self.images
    }

    pub const fn viewport(self) -> ViewportKey {
        self.viewport
    }
}

impl TakumiPaintCacheKey {
    pub const fn scene(self) -> TakumiSceneCacheKey {
        self.scene
    }

    pub const fn renderer_resources(self) -> RendererResourceRevision {
        self.renderer_resources
    }
}

fn milli_key(value: f32) -> u32 {
    if value.is_finite() && value > 0.0 {
        (value * 1000.0).round().to_u32().unwrap_or(u32::MAX)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_keys_split_layout_scene_from_renderer_resource_revisions() {
        let viewport = ViewportKey::new(1280, 720, 1.25, 16.0);
        let scene = TakumiSceneCacheKey::new(
            UiProgramRevision(1),
            ViewFragmentRevision(2),
            StyleRevision(3),
            TextLayoutRevision(4),
            ImageRevision(5),
            viewport,
        );
        let paint_a = scene.paint_only_key(RendererResourceRevision(6));
        let paint_b = scene.paint_only_key(RendererResourceRevision(7));

        assert_eq!(paint_a.scene(), scene);
        assert_ne!(paint_a, paint_b);
        assert_eq!(scene.viewport().device_pixel_ratio_milli(), 1250);
    }

    #[test]
    fn mask_image_url_resource_revision_changes_scene_cache_key() {
        let viewport = ViewportKey::new(1280, 720, 1.0, 16.0);
        let before_mask_url = TakumiSceneCacheKey::new(
            UiProgramRevision(1),
            ViewFragmentRevision(2),
            StyleRevision(3),
            TextLayoutRevision(4),
            ImageRevision(10),
            viewport,
        );
        let after_mask_url = TakumiSceneCacheKey::new(
            UiProgramRevision(1),
            ViewFragmentRevision(2),
            StyleRevision(3),
            TextLayoutRevision(4),
            ImageRevision(11),
            viewport,
        );

        assert_ne!(before_mask_url, after_mask_url);
        assert_eq!(before_mask_url.image_revision(), ImageRevision(10));
        assert_eq!(after_mask_url.image_revision(), ImageRevision(11));
    }
}
