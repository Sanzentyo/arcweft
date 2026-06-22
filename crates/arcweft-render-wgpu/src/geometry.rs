use crate::convert::usize_to_f32;
use arcweft_id::PublicId;
use arcweft_presentation::hit::{HitRect, HitTree};
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::{
    LayerContent, LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree,
    RenderPhase,
};
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use thiserror::Error;

/// Logical viewport shared by visual planning and hit-testing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderViewport {
    pub logical_width: f32,
    pub logical_height: f32,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f64,
}

/// User-facing presentation preferences independent of platform APIs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPreferences {
    pub text_scale_milli: u16,
    pub high_contrast: bool,
    pub reduce_motion: bool,
}

/// Choice list scroll state in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChoiceScroll {
    pub offset_y: f32,
}

/// Frame-crossing interaction visuals rendered into the canvas.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InteractionVisualState {
    pub focused: Option<InteractionTarget>,
    pub hovered: Option<InteractionTarget>,
    pub pressed: Option<InteractionTarget>,
}

/// Renderer input assembled by the player from portable runtime state.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderScene {
    pub dialogue: Option<RenderDialogue>,
    pub choices: Vec<RenderChoiceItem>,
    pub images: Vec<RenderImage>,
    pub viewport: RenderViewport,
    pub preferences: RenderPreferences,
    pub interaction: InteractionVisualState,
    pub choice_scroll: ChoiceScroll,
}

/// Minimal dialogue data consumed by the shared renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderDialogue {
    pub speaker: String,
    pub text: String,
}

/// Portable choice data supplied by a player/runtime adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderChoiceItem {
    pub id: String,
    pub label: String,
}

/// One colored rectangle in logical viewport coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintRect {
    pub bounds: HitRect,
    pub rgba: [f32; 4],
}

/// One decoded RGBA image frame ready for GPU upload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderImageFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One textured image quad in logical viewport coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderImage {
    pub id: String,
    pub frame: RenderImageFrame,
    pub bounds: HitRect,
    pub opacity_milli: u16,
}

/// One text block prepared for glyphon.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderTextBlock {
    pub text: String,
    pub bounds: HitRect,
    pub font_size: f32,
    pub line_height: f32,
    pub rgba: [u8; 4],
}

/// Choice geometry and stable semantic target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderChoice {
    pub option_id: String,
    pub label: String,
    pub target: InteractionTarget,
}

/// Pure frame plan consumed by the shared GPU renderer and input router.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFrame {
    pub viewport: RenderViewport,
    pub layers: LayerTree,
    pub semantics: SemanticTree,
    pub hits: HitTree,
    pub rectangles: Vec<PaintRect>,
    pub images: Vec<RenderImage>,
    pub text: Vec<RenderTextBlock>,
    pub choices: Vec<RenderChoice>,
}

/// Pure geometry planner shared by native and browser hosts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SharedFramePlanner;

/// Invalid frame inputs rejected before GPU work.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum FramePlanError {
    #[error("viewport must have non-zero logical and physical dimensions")]
    EmptyViewport,
    #[error("failed to construct stable presentation id `{value}`")]
    InvalidId { value: String },
}

impl Default for RenderPreferences {
    fn default() -> Self {
        Self {
            text_scale_milli: 1_000,
            high_contrast: false,
            reduce_motion: false,
        }
    }
}

impl SharedFramePlanner {
    /// # Panics
    ///
    /// Panics if internal layer parent ids are inconsistent. That indicates a
    /// planner bug rather than invalid caller input.
    pub fn prepare(scene: &RenderScene) -> Result<PreparedFrame, FramePlanError> {
        validate_viewport(scene.viewport)?;
        let ids = FrameIds::new()?;
        let mut layers = LayerTree::new(
            LayerNode::new(
                ids.root.clone(),
                LayerKind::Root,
                order(RenderPhase::Background, 0),
            )
            .with_input_policy(LayerInputPolicy::Ignore),
        );
        layers
            .insert(
                LayerNode::new(
                    ids.dialogue.clone(),
                    LayerKind::TextBox,
                    order(RenderPhase::Dialogue, 0),
                )
                .with_parent(ids.root.clone())
                .with_content(LayerContent::TextBox(ids.dialogue_content.clone()))
                .with_input_policy(LayerInputPolicy::Ignore),
            )
            .expect("dialogue layer parent is present");
        layers
            .insert(
                LayerNode::new(
                    ids.choice.clone(),
                    LayerKind::GameUi,
                    order(RenderPhase::GameUi, 0),
                )
                .with_parent(ids.root)
                .with_content(LayerContent::NativeUi(ids.choice_content.clone()))
                .with_input_policy(LayerInputPolicy::HitTest),
            )
            .expect("choice layer parent is present");

        let palette = Palette::from_preferences(scene.preferences);
        let mut rectangles = vec![PaintRect {
            bounds: HitRect::new(
                0.0,
                0.0,
                scene.viewport.logical_width,
                scene.viewport.logical_height,
            ),
            rgba: palette.background,
        }];
        let mut text = Vec::new();
        if let Some(dialogue) = &scene.dialogue {
            let panel = dialogue_panel(scene.viewport);
            rectangles.push(PaintRect {
                bounds: panel,
                rgba: palette.dialogue_panel,
            });
            let inset = 28.0;
            let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
            text.push(RenderTextBlock {
                text: dialogue.speaker.clone(),
                bounds: HitRect::new(
                    panel.x + inset,
                    panel.y + 20.0,
                    panel.width - inset * 2.0,
                    28.0 * scale,
                ),
                font_size: 20.0 * scale,
                line_height: 26.0 * scale,
                rgba: palette.speaker_text,
            });
            text.push(RenderTextBlock {
                text: dialogue.text.clone(),
                bounds: HitRect::new(
                    panel.x + inset,
                    panel.y + 58.0,
                    panel.width - inset * 2.0,
                    panel.height - 76.0,
                ),
                font_size: 25.0 * scale,
                line_height: 34.0 * scale,
                rgba: palette.dialogue_text,
            });
        }

        let mut semantics = SemanticTree::default();
        let action = RenderActionKind::ChoiceSelect.public_id()?;
        let choices = build_choices(
            scene,
            &ids.choice,
            &mut semantics,
            &mut rectangles,
            &mut text,
            &palette,
            &action,
        )?;
        let hits = semantics.to_hit_tree();

        Ok(PreparedFrame {
            viewport: scene.viewport,
            layers,
            semantics,
            hits,
            rectangles,
            images: scene.images.clone(),
            text,
            choices,
        })
    }
}

impl PreparedFrame {
    pub fn choice_for_target(&self, target: &InteractionTarget) -> Option<&RenderChoice> {
        self.choices.iter().find(|choice| &choice.target == target)
    }

    pub fn first_choice_target(&self) -> Option<InteractionTarget> {
        self.choices.first().map(|choice| choice.target.clone())
    }

    pub fn last_choice_target(&self) -> Option<InteractionTarget> {
        self.choices.last().map(|choice| choice.target.clone())
    }

    pub fn adjacent_choice_target(
        &self,
        current: Option<&InteractionTarget>,
        delta: isize,
    ) -> Option<InteractionTarget> {
        if self.choices.is_empty() {
            return None;
        }
        let current = current
            .and_then(|target| {
                self.choices
                    .iter()
                    .position(|choice| &choice.target == target)
            })
            .unwrap_or(0);
        let len = isize::try_from(self.choices.len()).ok()?;
        let next = (isize::try_from(current).ok()? + delta).rem_euclid(len);
        self.choices
            .get(usize::try_from(next).ok()?)
            .map(|choice| choice.target.clone())
    }
}

fn build_choices(
    scene: &RenderScene,
    layer: &LayerId,
    semantics: &mut SemanticTree,
    rectangles: &mut Vec<PaintRect>,
    text: &mut Vec<RenderTextBlock>,
    palette: &Palette,
    action: &PublicId,
) -> Result<Vec<RenderChoice>, FramePlanError> {
    let width = (scene.viewport.logical_width * 0.64).clamp(320.0, 920.0);
    let item_height = 58.0;
    let gap = 12.0;
    let total = usize_to_f32(scene.choices.len()) * (item_height + gap) - gap;
    let top =
        ((scene.viewport.logical_height - total) * 0.42).max(36.0) - scene.choice_scroll.offset_y;
    let left = (scene.viewport.logical_width - width) * 0.5;
    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;

    scene
        .choices
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            let target = InteractionTarget::new(ChoiceTargetId(index).public_id()?);
            let bounds = HitRect::new(
                left,
                top + usize_to_f32(index) * (item_height + gap),
                width,
                item_height,
            );
            let is_focused = scene.interaction.focused.as_ref() == Some(&target);
            let is_hovered = scene.interaction.hovered.as_ref() == Some(&target);
            let is_pressed = scene.interaction.pressed.as_ref() == Some(&target);
            rectangles.push(PaintRect {
                bounds,
                rgba: if is_pressed {
                    palette.choice_pressed
                } else if is_focused || is_hovered {
                    palette.choice_active
                } else {
                    palette.choice_idle
                },
            });
            if is_focused {
                push_focus_ring(rectangles, bounds, palette.focus_ring);
            }
            text.push(RenderTextBlock {
                text: choice.label.clone(),
                bounds: HitRect::new(
                    bounds.x + 20.0,
                    bounds.y + 13.0,
                    bounds.width - 40.0,
                    bounds.height - 20.0,
                ),
                font_size: 22.0 * scale,
                line_height: 29.0 * scale,
                rgba: palette.choice_text,
            });
            semantics.push(
                SemanticNode::new(layer.clone(), target.clone(), SemanticRole::Button, bounds)
                    .with_label(choice.label.clone())
                    .with_action(action.clone()),
            );
            Ok(RenderChoice {
                option_id: choice.id.clone(),
                label: choice.label.clone(),
                target,
            })
        })
        .collect()
}

fn push_focus_ring(rectangles: &mut Vec<PaintRect>, bounds: HitRect, color: [f32; 4]) {
    let thickness = 3.0;
    rectangles.extend([
        PaintRect {
            bounds: HitRect::new(bounds.x, bounds.y, bounds.width, thickness),
            rgba: color,
        },
        PaintRect {
            bounds: HitRect::new(
                bounds.x,
                bounds.y + bounds.height - thickness,
                bounds.width,
                thickness,
            ),
            rgba: color,
        },
        PaintRect {
            bounds: HitRect::new(bounds.x, bounds.y, thickness, bounds.height),
            rgba: color,
        },
        PaintRect {
            bounds: HitRect::new(
                bounds.x + bounds.width - thickness,
                bounds.y,
                thickness,
                bounds.height,
            ),
            rgba: color,
        },
    ]);
}

fn dialogue_panel(viewport: RenderViewport) -> HitRect {
    let margin = (viewport.logical_width * 0.045).max(24.0);
    let height = (viewport.logical_height * 0.28).clamp(180.0, 320.0);
    HitRect::new(
        margin,
        viewport.logical_height - height - margin,
        viewport.logical_width - margin * 2.0,
        height,
    )
}

fn validate_viewport(viewport: RenderViewport) -> Result<(), FramePlanError> {
    (viewport.logical_width > 0.0
        && viewport.logical_height > 0.0
        && viewport.physical_width > 0
        && viewport.physical_height > 0)
        .then_some(())
        .ok_or(FramePlanError::EmptyViewport)
}

const fn order(phase: RenderPhase, z: i32) -> LayerOrder {
    LayerOrder {
        phase,
        z,
        stable_index: 0,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderActionKind {
    ChoiceSelect,
}

impl RenderActionKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ChoiceSelect => "action.choice.select",
        }
    }

    fn public_id(self) -> Result<PublicId, FramePlanError> {
        PublicId::try_new(self.as_str()).map_err(|_| FramePlanError::InvalidId {
            value: self.as_str().to_owned(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameStaticId {
    RootLayer,
    DialogueLayer,
    ChoiceLayer,
    DialogueContent,
    ChoiceContent,
}

impl FrameStaticId {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RootLayer => "layer.player.root",
            Self::DialogueLayer => "layer.player.dialogue",
            Self::ChoiceLayer => "layer.player.choice",
            Self::DialogueContent => "textbox.player.dialogue",
            Self::ChoiceContent => "ui.player.choice",
        }
    }

    fn public_id(self) -> Result<PublicId, FramePlanError> {
        PublicId::try_new(self.as_str()).map_err(|_| FramePlanError::InvalidId {
            value: self.as_str().to_owned(),
        })
    }

    fn layer_id(self) -> Result<LayerId, FramePlanError> {
        self.public_id().map(LayerId::new)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChoiceTargetId(usize);

impl ChoiceTargetId {
    fn public_id(self) -> Result<PublicId, FramePlanError> {
        let value = format!("target.choice.{}", self.0);
        PublicId::try_new(&value).map_err(|_| FramePlanError::InvalidId { value })
    }
}

struct FrameIds {
    root: LayerId,
    dialogue: LayerId,
    choice: LayerId,
    dialogue_content: PublicId,
    choice_content: PublicId,
}

impl FrameIds {
    fn new() -> Result<Self, FramePlanError> {
        Ok(Self {
            root: FrameStaticId::RootLayer.layer_id()?,
            dialogue: FrameStaticId::DialogueLayer.layer_id()?,
            choice: FrameStaticId::ChoiceLayer.layer_id()?,
            dialogue_content: FrameStaticId::DialogueContent.public_id()?,
            choice_content: FrameStaticId::ChoiceContent.public_id()?,
        })
    }
}

struct Palette {
    background: [f32; 4],
    dialogue_panel: [f32; 4],
    choice_idle: [f32; 4],
    choice_active: [f32; 4],
    choice_pressed: [f32; 4],
    focus_ring: [f32; 4],
    speaker_text: [u8; 4],
    dialogue_text: [u8; 4],
    choice_text: [u8; 4],
}

impl Palette {
    fn from_preferences(preferences: RenderPreferences) -> Self {
        if preferences.high_contrast {
            Self {
                background: [0.0, 0.0, 0.0, 1.0],
                dialogue_panel: [0.02, 0.02, 0.02, 0.98],
                choice_idle: [0.08, 0.08, 0.08, 1.0],
                choice_active: [0.2, 0.2, 0.2, 1.0],
                choice_pressed: [0.32, 0.32, 0.32, 1.0],
                focus_ring: [1.0, 1.0, 0.0, 1.0],
                speaker_text: [255, 255, 0, 255],
                dialogue_text: [255, 255, 255, 255],
                choice_text: [255, 255, 255, 255],
            }
        } else {
            Self {
                background: [0.018, 0.024, 0.05, 1.0],
                dialogue_panel: [0.035, 0.055, 0.11, 0.94],
                choice_idle: [0.08, 0.11, 0.20, 0.96],
                choice_active: [0.14, 0.24, 0.42, 1.0],
                choice_pressed: [0.11, 0.18, 0.32, 1.0],
                focus_ring: [0.46, 0.79, 1.0, 1.0],
                speaker_text: [139, 211, 255, 255],
                dialogue_text: [241, 246, 255, 255],
                choice_text: [246, 249, 255, 255],
            }
        }
    }
}
