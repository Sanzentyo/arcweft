use crate::convert::saturating_usize_as_f32;
use arcweft_id::PublicId;
use arcweft_presentation::hit::{HitRect, HitTree};
use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform};
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::{
    LayerContent, LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree,
    RenderPhase,
};
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use arcweft_presentation::text_editor::TextEditorError;
use arcweft_presentation::text_input::{TextInputClientSnapshot, TextInputGeometrySnapshot};
use arcweft_render_text::{
    LineDisplayFrame, RichTextColor, RichTextEffectDescriptor, RichTextEffectPhase,
    RichTextFontFamily, RichTextParam, RichTextRange, RichTextStyle, RichTextTextRun,
    presentation_from_styles,
};
use num_traits::ToPrimitive;
use thiserror::Error;

mod text_controls;
pub use text_controls::RenderTextInputControl;

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
    pub text_inputs: Vec<RenderTextInputControl>,
    pub images: Vec<RenderImage>,
    pub viewport: RenderViewport,
    pub visual_time_millis: u64,
    pub preferences: RenderPreferences,
    pub interaction: InteractionVisualState,
    pub choice_scroll: ChoiceScroll,
}

/// Minimal dialogue data consumed by the shared renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderDialogue {
    pub speaker: String,
    pub text: String,
    pub base_styles: Vec<RichTextStyle>,
    pub text_runs: Vec<RichTextTextRun>,
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
    pub fit: ImageObjectFit,
    pub alignment: ImageObjectAlignment,
    pub transform: ImageObjectTransform,
    pub opacity_milli: u16,
}

/// One text block prepared for glyphon.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderTextBlock {
    pub text: String,
    pub bounds: HitRect,
    pub font_size: f32,
    pub line_height: f32,
    pub font_family: RenderFontFamily,
    pub weight: RenderTextWeight,
    pub slant: RenderTextSlant,
    pub rgba: [u8; 4],
}

/// Font family requested by a prepared text block.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum RenderFontFamily {
    Serif,
    #[default]
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    Named(String),
}

impl RenderFontFamily {
    fn from_rich_text(family: &RichTextFontFamily) -> Self {
        match family {
            RichTextFontFamily::Serif => Self::Serif,
            RichTextFontFamily::SansSerif => Self::SansSerif,
            RichTextFontFamily::Monospace => Self::Monospace,
            RichTextFontFamily::Cursive => Self::Cursive,
            RichTextFontFamily::Fantasy => Self::Fantasy,
            RichTextFontFamily::Named { name } => Self::Named(name.clone()),
        }
    }
}

/// Text weight requested by a prepared text block.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderTextWeight {
    #[default]
    Regular,
    Bold,
}

/// Text slant requested by a prepared text block.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderTextSlant {
    #[default]
    Upright,
    Italic,
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
    focused_text_input: Option<PreparedTextInputTarget>,
}

/// Renderer-backed text input target prepared for platform IME adapters.
///
/// This intentionally contains Arcweft-owned text-input snapshots rather than
/// native handles. Platform adapters consume it through the native player
/// bridge.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTextInputTarget {
    pub snapshot: TextInputClientSnapshot,
    pub geometry: TextInputGeometrySnapshot,
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
    #[error("semantic role {role:?} is not a text-input control")]
    InvalidTextInputRole { role: SemanticRole },
    #[error(transparent)]
    TextEditor(#[from] TextEditorError),
}

impl PreparedFrame {
    /// Returns the renderer-backed focused text target for platform IME sync.
    ///
    /// This is the single native/web focus-target source. It is populated only
    /// by real `RenderTextInputControl` input lowered from runtime/player state.
    #[must_use]
    pub fn focused_text_input_target(&self) -> Option<PreparedTextInputTarget> {
        self.focused_text_input.clone()
    }
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
                .with_parent(ids.root.clone())
                .with_content(LayerContent::NativeUi(ids.choice_content.clone()))
                .with_input_policy(LayerInputPolicy::HitTest),
            )
            .expect("choice layer parent is present");
        layers
            .insert(
                LayerNode::new(
                    ids.text_input.clone(),
                    LayerKind::GameUi,
                    order(RenderPhase::GameUi, 1),
                )
                .with_parent(ids.root)
                .with_content(LayerContent::NativeUi(ids.text_input_content.clone()))
                .with_input_policy(LayerInputPolicy::HitTest),
            )
            .expect("text-input layer parent is present");

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
        push_dialogue_panel(scene, &mut rectangles, &mut text, &palette);

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
        let focused_text_input = text_controls::build_text_inputs(
            scene,
            &ids.text_input,
            &mut semantics,
            &mut rectangles,
            &mut text,
            &palette,
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
            focused_text_input,
        })
    }
}

fn push_dialogue_panel(
    scene: &RenderScene,
    rectangles: &mut Vec<PaintRect>,
    text: &mut Vec<RenderTextBlock>,
    palette: &Palette,
) {
    let Some(dialogue) = &scene.dialogue else {
        return;
    };
    let panel = dialogue_panel(scene.viewport);
    rectangles.push(PaintRect {
        bounds: panel,
        rgba: palette.dialogue_panel,
    });
    let inset = 28.0;
    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    let base_style = text_style_from_styles(
        &dialogue.base_styles,
        RenderTextStyle::new(
            25.0 * scale,
            34.0 * scale,
            palette.dialogue_text,
            RenderFontFamily::SansSerif,
        ),
    );
    let speaker_style = RenderTextStyle {
        font_size: (base_style.font_size * 0.8).max(16.0 * scale),
        line_height: (base_style.line_height * 0.78).max(24.0 * scale),
        color: base_style.color,
        font_family: base_style.font_family.clone(),
        weight: RenderTextWeight::Bold,
        slant: base_style.slant,
    };
    text.push(RenderTextBlock {
        text: dialogue.speaker.clone(),
        bounds: HitRect::new(
            panel.x + inset,
            panel.y + 20.0,
            panel.width - inset * 2.0,
            28.0 * scale,
        ),
        font_size: speaker_style.font_size,
        line_height: speaker_style.line_height,
        font_family: speaker_style.font_family,
        weight: speaker_style.weight,
        slant: speaker_style.slant,
        rgba: if dialogue.base_styles.is_empty() {
            palette.speaker_text
        } else {
            speaker_style.color
        },
    });
    push_dialogue_text_blocks(
        text,
        dialogue,
        &DialogueTextLayout {
            bounds: HitRect::new(
                panel.x + inset,
                panel.y + 58.0,
                panel.width - inset * 2.0,
                panel.height - 76.0,
            ),
            style: base_style,
            visual_time_millis: scene.visual_time_millis,
            reduce_motion: scene.preferences.reduce_motion,
        },
    );
}

impl RenderDialogue {
    pub fn plain(speaker: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            speaker: speaker.into(),
            text: text.into(),
            base_styles: Vec::new(),
            text_runs: Vec::new(),
        }
    }

    pub fn from_display_frame(frame: &LineDisplayFrame) -> Self {
        Self {
            speaker: frame.callee.clone(),
            text: frame.text.clone(),
            base_styles: frame.base_styles.clone(),
            text_runs: frame.display_map.text_runs.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DialogueTextLayout {
    bounds: HitRect,
    style: RenderTextStyle,
    visual_time_millis: u64,
    reduce_motion: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct RenderTextStyle {
    font_size: f32,
    line_height: f32,
    color: [u8; 4],
    font_family: RenderFontFamily,
    weight: RenderTextWeight,
    slant: RenderTextSlant,
}

impl RenderTextStyle {
    const fn new(
        font_size: f32,
        line_height: f32,
        color: [u8; 4],
        font_family: RenderFontFamily,
    ) -> Self {
        Self {
            font_size,
            line_height,
            color,
            font_family,
            weight: RenderTextWeight::Regular,
            slant: RenderTextSlant::Upright,
        }
    }
}

fn push_dialogue_text_blocks(
    text: &mut Vec<RenderTextBlock>,
    dialogue: &RenderDialogue,
    layout: &DialogueTextLayout,
) {
    let runs = if dialogue.text_runs.is_empty() {
        let styles = dialogue.base_styles.clone();
        vec![RichTextTextRun {
            range: RichTextRange::new(0, dialogue.text.len()),
            source: arcweft_render_text::RichTextTextSource::Text,
            node_index: 0,
            presentation: presentation_from_styles(&styles),
            styles,
        }]
    } else {
        dialogue.text_runs.clone()
    };
    let visible_end = if layout.reduce_motion {
        dialogue.text.len()
    } else {
        typewriter_visible_end(&dialogue.text, &runs, layout.visual_time_millis)
    };
    let mut offset_x: f32 = 0.0;
    for run in runs {
        let run_start = run.range.start.min(dialogue.text.len());
        let run_end = run.range.end.min(dialogue.text.len()).min(visible_end);
        if run_start >= run_end {
            continue;
        }
        let Some(visible) = dialogue.text.get(run_start..run_end) else {
            continue;
        };
        let run_style = text_style_from_styles(&run.styles, layout.style.clone());
        let x = layout.bounds.x + offset_x.min(layout.bounds.width);
        let bounds = HitRect::new(
            x,
            layout.bounds.y,
            (layout.bounds.width - (x - layout.bounds.x)).max(1.0),
            layout.bounds.height,
        );
        let motion = (!layout.reduce_motion)
            .then(|| text_motion(&run.presentation.effects))
            .flatten();
        if let Some(motion) = motion {
            push_motion_text_blocks(text, visible, bounds, layout, &run_style, motion, run_start);
        } else {
            text.push(RenderTextBlock {
                text: visible.to_owned(),
                bounds,
                font_size: run_style.font_size,
                line_height: run_style.line_height,
                font_family: run_style.font_family.clone(),
                weight: run_style.weight,
                slant: run_style.slant,
                rgba: run_style.color,
            });
        }
        offset_x += estimated_text_width(visible, run_style.font_size);
        if offset_x >= layout.bounds.width {
            break;
        }
    }
}

fn typewriter_visible_end(text: &str, runs: &[RichTextTextRun], visual_time_millis: u64) -> usize {
    const DEFAULT_TYPEWRITER_CPS: f32 = 28.0;
    let cps = runs
        .iter()
        .flat_map(|run| &run.presentation.effects)
        .find(|effect| effect.id == "typewriter" && effect.phase == RichTextEffectPhase::GlyphMask)
        .map_or(DEFAULT_TYPEWRITER_CPS, typewriter_cps);
    let visible_chars = ((visual_time_millis.to_f32().unwrap_or(f32::MAX) / 1_000.0) * cps)
        .floor()
        .to_usize()
        .unwrap_or(usize::MAX);
    text.char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .nth(visible_chars)
        .unwrap_or(text.len())
}

fn typewriter_cps(effect: &RichTextEffectDescriptor) -> f32 {
    effect
        .params
        .get("cps")
        .or_else(|| effect.params.get("speed"))
        .and_then(param_f32)
        .unwrap_or(28.0)
        .clamp(1.0, 240.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TextMotion {
    amplitude: f32,
    frequency: f32,
}

fn text_motion(effects: &[RichTextEffectDescriptor]) -> Option<TextMotion> {
    effects
        .iter()
        .find(|effect| {
            matches!(effect.id.as_str(), "wave" | "shake" | "jitter")
                && effect.phase == RichTextEffectPhase::GlyphTransform
        })
        .map(|effect| TextMotion {
            amplitude: effect
                .params
                .get("amp")
                .or_else(|| effect.params.get("amplitude"))
                .and_then(param_f32)
                .unwrap_or(4.0)
                .clamp(0.0, 24.0),
            frequency: effect
                .params
                .get("freq")
                .or_else(|| effect.params.get("frequency"))
                .and_then(param_f32)
                .unwrap_or(7.0)
                .clamp(0.1, 24.0),
        })
}

fn push_motion_text_blocks(
    text: &mut Vec<RenderTextBlock>,
    visible: &str,
    bounds: HitRect,
    layout: &DialogueTextLayout,
    style: &RenderTextStyle,
    motion: TextMotion,
    range_start: usize,
) {
    let seconds = layout.visual_time_millis.to_f32().unwrap_or(f32::MAX) / 1_000.0;
    let mut offset_x = 0.0;
    for (index, ch) in visible.chars().enumerate() {
        let advance = estimated_char_width(ch, style.font_size);
        let phase =
            seconds * motion.frequency + (range_start + index).to_f32().unwrap_or(f32::MAX) * 0.58;
        let offset_y = if ch.is_whitespace() {
            0.0
        } else {
            phase.sin() * motion.amplitude
        };
        text.push(RenderTextBlock {
            text: ch.to_string(),
            bounds: HitRect::new(
                bounds.x + offset_x,
                bounds.y + offset_y,
                advance.max(1.0),
                bounds.height,
            ),
            font_size: style.font_size,
            line_height: style.line_height,
            font_family: style.font_family.clone(),
            weight: style.weight,
            slant: style.slant,
            rgba: style.color,
        });
        offset_x += advance;
        if offset_x >= bounds.width {
            break;
        }
    }
}

fn text_style_from_styles(styles: &[RichTextStyle], fallback: RenderTextStyle) -> RenderTextStyle {
    styles.iter().fold(fallback, apply_text_style)
}

fn apply_text_style(mut style: RenderTextStyle, rich_style: &RichTextStyle) -> RenderTextStyle {
    match rich_style {
        RichTextStyle::Em { .. } | RichTextStyle::Italic { .. } | RichTextStyle::Oblique { .. } => {
            style.slant = RenderTextSlant::Italic;
        }
        RichTextStyle::Strong { .. } => style.weight = RenderTextWeight::Bold,
        RichTextStyle::Color { value } => style.color = rich_text_color(value),
        RichTextStyle::Font { family } => {
            style.font_family = RenderFontFamily::from_rich_text(family);
        }
        RichTextStyle::Size {
            points: Some(points),
            ..
        } => {
            style.font_size = f32::from(*points);
            style.line_height = style.font_size * 1.35;
        }
        RichTextStyle::Size { points: None, .. }
        | RichTextStyle::Speed { .. }
        | RichTextStyle::Layout { .. }
        | RichTextStyle::Transform { .. }
        | RichTextStyle::Presentation { .. }
        | RichTextStyle::Effect { .. }
        | RichTextStyle::Shader { .. }
        | RichTextStyle::Object { .. }
        | RichTextStyle::Unknown { .. } => {}
    }
    style
}

fn rich_text_color(color: &RichTextColor) -> [u8; 4] {
    match color {
        RichTextColor::Rgb { red, green, blue } => [*red, *green, *blue, 255],
        RichTextColor::Named { name } => match name.as_str() {
            "red" => [240, 110, 110, 255],
            "green" => [120, 220, 150, 255],
            "blue" => [130, 180, 255, 255],
            "yellow" => [240, 220, 120, 255],
            "muted" | "quiet" => [170, 170, 170, 255],
            _ => [245, 245, 245, 255],
        },
    }
}

fn param_f32(param: &RichTextParam) -> Option<f32> {
    match param {
        RichTextParam::Int { value } => value.to_f32(),
        RichTextParam::Milli { value } => Some(value.as_f32()),
        RichTextParam::Text { value } | RichTextParam::Raw { value } => {
            value.trim().trim_end_matches("px").parse().ok()
        }
        RichTextParam::Bool { .. }
        | RichTextParam::Vec2 { .. }
        | RichTextParam::Selector { .. }
        | RichTextParam::Expr { .. } => None,
    }
}

fn estimated_text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|ch| estimated_char_width(ch, font_size))
        .sum()
}

fn estimated_char_width(ch: char, font_size: f32) -> f32 {
    let ratio = if ch.is_ascii_whitespace() {
        0.34
    } else if ch.is_ascii() {
        0.58
    } else {
        0.94
    };
    font_size * ratio
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
    if scene.choices.is_empty() {
        return Ok(Vec::new());
    }
    let width = (scene.viewport.logical_width * 0.52).clamp(360.0, 760.0);
    let item_height = 60.0;
    let gap = 12.0;
    let total = saturating_usize_as_f32(scene.choices.len()) * (item_height + gap) - gap;
    let top = scene.dialogue.as_ref().map_or_else(
        || ((scene.viewport.logical_height - total) * 0.42).max(36.0),
        |_| {
            let panel = dialogue_panel(scene.viewport);
            (panel.y - total - 22.0).max(36.0)
        },
    );
    let left = (scene.viewport.logical_width - width) * 0.5;
    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    let font_size = 22.0 * scale;
    let line_height = 30.0 * scale;

    scene
        .choices
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            let target = InteractionTarget::new(ChoiceTargetId(index).public_id()?);
            let bounds = HitRect::new(
                left,
                top + saturating_usize_as_f32(index) * (item_height + gap),
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
                    bounds.x + 24.0,
                    bounds.y + (bounds.height - line_height) * 0.5,
                    bounds.width - 48.0,
                    line_height,
                ),
                font_size,
                line_height,
                font_family: RenderFontFamily::SansSerif,
                weight: RenderTextWeight::Bold,
                slant: RenderTextSlant::Upright,
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
    TextInputLayer,
    DialogueContent,
    ChoiceContent,
    TextInputContent,
}

impl FrameStaticId {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RootLayer => "layer.player.root",
            Self::DialogueLayer => "layer.player.dialogue",
            Self::ChoiceLayer => "layer.player.choice",
            Self::TextInputLayer => "layer.player.text_input",
            Self::DialogueContent => "textbox.player.dialogue",
            Self::ChoiceContent => "ui.player.choice",
            Self::TextInputContent => "ui.player.text_input",
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
    text_input: LayerId,
    dialogue_content: PublicId,
    choice_content: PublicId,
    text_input_content: PublicId,
}

impl FrameIds {
    fn new() -> Result<Self, FramePlanError> {
        Ok(Self {
            root: FrameStaticId::RootLayer.layer_id()?,
            dialogue: FrameStaticId::DialogueLayer.layer_id()?,
            choice: FrameStaticId::ChoiceLayer.layer_id()?,
            text_input: FrameStaticId::TextInputLayer.layer_id()?,
            dialogue_content: FrameStaticId::DialogueContent.public_id()?,
            choice_content: FrameStaticId::ChoiceContent.public_id()?,
            text_input_content: FrameStaticId::TextInputContent.public_id()?,
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
                background: [0.019, 0.027, 0.024, 1.0],
                dialogue_panel: [0.066, 0.071, 0.064, 0.95],
                choice_idle: [0.125, 0.124, 0.099, 0.98],
                choice_active: [0.119, 0.235, 0.153, 1.0],
                choice_pressed: [0.207, 0.3, 0.164, 1.0],
                focus_ring: [0.886, 0.914, 0.384, 1.0],
                speaker_text: [174, 226, 142, 255],
                dialogue_text: [248, 246, 234, 255],
                choice_text: [255, 252, 238, 255],
            }
        }
    }
}
