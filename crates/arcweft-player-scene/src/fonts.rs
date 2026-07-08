//! Shared player font registration for renderer-exact frame planning.

use crate::frame::{PlayerFrameError, PlayerFramePlannerState};
use arcweft_render_wgpu::offscreen::{SharedOffscreenCapture, SharedOffscreenCaptureError};
use arcweft_render_wgpu::renderer::{SharedRenderer, SharedRendererError};
use thiserror::Error;

/// Font bytes bundled with the stock native/Web player shell.
pub const DEFAULT_PLAYER_FONT_BYTES: &[u8] = include_bytes!("../../../web/assets/arcweft-demo.ttf");

/// Ordered project/player font bytes applied to both planning and rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerFontSet {
    fonts: Vec<Vec<u8>>,
}

/// Error returned when a player font set cannot be registered consistently.
#[derive(Debug, Error)]
pub enum PlayerFontRegistrationError {
    #[error("player font set must contain at least one font")]
    EmptySet,
    #[error("font resource {index} must not be empty")]
    EmptyFontResource { index: usize },
    #[error("font resource {index} decode failed: {source}")]
    DecodeFontResource {
        index: usize,
        #[source]
        source: oxifont_webfont::WebFontError,
    },
    #[error("frame planner font registration failed: {0}")]
    Planner(#[from] PlayerFrameError),
    #[error("renderer font registration failed: {0}")]
    Renderer(#[from] SharedRendererError),
    #[error("offscreen renderer font registration failed: {0}")]
    Offscreen(#[from] SharedOffscreenCaptureError),
}

impl PlayerFontSet {
    #[must_use]
    pub fn new(fonts: Vec<Vec<u8>>) -> Self {
        Self { fonts }
    }

    #[must_use]
    pub fn bundled_default() -> Self {
        Self::new(vec![DEFAULT_PLAYER_FONT_BYTES.to_vec()])
    }

    #[must_use]
    pub fn single(bytes: Vec<u8>) -> Self {
        Self::new(vec![bytes])
    }

    pub fn from_font_resource_bytes(
        fonts: Vec<Vec<u8>>,
    ) -> Result<Self, PlayerFontRegistrationError> {
        if fonts.is_empty() {
            return Err(PlayerFontRegistrationError::EmptySet);
        }
        fonts
            .into_iter()
            .enumerate()
            .map(|(index, bytes)| {
                if bytes.is_empty() {
                    return Err(PlayerFontRegistrationError::EmptyFontResource { index });
                }
                oxifont_webfont::decode_auto(&bytes)
                    .map(|decoded| decoded.sfnt)
                    .map_err(|source| PlayerFontRegistrationError::DecodeFontResource {
                        index,
                        source,
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self::new)
    }

    #[must_use]
    pub fn fonts(&self) -> &[Vec<u8>] {
        &self.fonts
    }

    pub fn register_with_planner(
        &self,
        planner: &mut PlayerFramePlannerState,
    ) -> Result<(), PlayerFontRegistrationError> {
        self.ensure_non_empty()?;
        self.fonts
            .iter()
            .try_for_each(|bytes| planner.register_font_bytes(bytes.clone()))?;
        Ok(())
    }

    pub fn register_with_renderer(
        &self,
        renderer: &mut SharedRenderer,
    ) -> Result<(), PlayerFontRegistrationError> {
        self.ensure_non_empty()?;
        self.fonts
            .iter()
            .try_for_each(|bytes| renderer.register_font_bytes(bytes.clone()))?;
        Ok(())
    }

    pub fn register_with_offscreen_capture(
        &self,
        capture: &mut SharedOffscreenCapture,
    ) -> Result<(), PlayerFontRegistrationError> {
        self.ensure_non_empty()?;
        self.fonts
            .iter()
            .try_for_each(|bytes| capture.register_font_bytes(bytes.clone()))?;
        Ok(())
    }

    pub fn register_with_renderer_and_planner(
        &self,
        renderer: &mut SharedRenderer,
        planner: &mut PlayerFramePlannerState,
    ) -> Result<(), PlayerFontRegistrationError> {
        self.register_with_renderer(renderer)?;
        self.register_with_planner(planner)
    }

    fn ensure_non_empty(&self) -> Result<(), PlayerFontRegistrationError> {
        if self.fonts.is_empty() {
            Err(PlayerFontRegistrationError::EmptySet)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PLAYER_FONT_BYTES, PlayerFontRegistrationError, PlayerFontSet};
    use crate::frame::PlayerFramePlannerState;

    #[test]
    fn bundled_default_registers_with_frame_planner() {
        let fonts = PlayerFontSet::bundled_default();
        let mut planner = PlayerFramePlannerState::new();

        fonts
            .register_with_planner(&mut planner)
            .expect("bundled default font registers");

        assert_eq!(
            planner.stats().registered_font_bytes,
            DEFAULT_PLAYER_FONT_BYTES.len()
        );
    }

    #[test]
    fn empty_font_set_is_rejected_before_registration() {
        let fonts = PlayerFontSet::new(Vec::new());
        let mut planner = PlayerFramePlannerState::new();

        assert!(matches!(
            fonts.register_with_planner(&mut planner),
            Err(PlayerFontRegistrationError::EmptySet)
        ));
    }

    #[test]
    fn sfnt_font_resource_bytes_are_accepted() {
        let fonts =
            PlayerFontSet::from_font_resource_bytes(vec![DEFAULT_PLAYER_FONT_BYTES.to_vec()])
                .expect("sfnt font resource bytes are accepted");

        assert_eq!(fonts.fonts().len(), 1);
    }

    #[test]
    fn web_default_japanese_font_resource_bytes_are_accepted() {
        let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../web/assets/noto-sans-jp-vf.ttf");
        let font_bytes = std::fs::read(font_path).expect("web default Japanese font asset exists");
        let fonts = PlayerFontSet::from_font_resource_bytes(vec![
            DEFAULT_PLAYER_FONT_BYTES.to_vec(),
            font_bytes,
        ])
        .expect("web default Japanese font resource bytes are accepted");

        assert_eq!(fonts.fonts().len(), 2);
    }

    #[test]
    fn empty_font_resource_is_rejected_with_index() {
        assert!(matches!(
            PlayerFontSet::from_font_resource_bytes(vec![
                DEFAULT_PLAYER_FONT_BYTES.to_vec(),
                Vec::new()
            ]),
            Err(PlayerFontRegistrationError::EmptyFontResource { index: 1 })
        ));
    }
}
