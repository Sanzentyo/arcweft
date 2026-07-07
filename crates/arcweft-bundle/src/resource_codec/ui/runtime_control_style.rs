use core::fmt;

use serde::{Deserialize, Serialize};

use super::model::{
    RgbaColor, StyleAssignOp, SystemColor, UiInputKind, UiInputResource, UiTextResource,
    ViewElementKind, ViewElementState, ViewInteractionState, ViewPartStyleRule,
    ViewProgramInstruction, ViewProgramResource, ViewRuntimeActionButton, ViewRuntimeTextBlock,
    ViewRuntimeTextControl, ViewStyleApplyRef, ViewStyleDeclaration, ViewStyleResource,
    ViewStyleSelector, ViewStyleSelectorPart, ViewStyleValue,
};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlStyle {
    #[serde(
        default,
        skip_serializing_if = "ViewRuntimeControlVisualStyle::is_default"
    )]
    pub normal: ViewRuntimeControlVisualStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hover: Option<ViewRuntimeControlVisualStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressed: Option<ViewRuntimeControlVisualStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_visible: Option<ViewRuntimeControlVisualStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<ViewRuntimeControlVisualStyle>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlVisualStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<RgbaColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<RgbaColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<RgbaColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caret: Option<RgbaColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<ViewRuntimeControlBorderStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_frame: Option<ViewRuntimeControlCornerFrameStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_ring: Option<ViewRuntimeControlFocusRingStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity_milli: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radii_milli: Option<ViewRuntimeControlRadii>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_milli: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<ViewRuntimeControlFilterList>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop_filters: Option<ViewRuntimeControlFilterList>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<ViewRuntimeShadow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlBorderStyle {
    pub color: RgbaColor,
    pub width_milli: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlCornerFrameStyle {
    pub color: RgbaColor,
    pub width_milli: u32,
    pub length_milli: u32,
    pub offset_milli: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlFocusRingStyle {
    pub color: RgbaColor,
    pub width_milli: u32,
    pub offset_milli: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlRadii {
    pub top_left: ViewRuntimeControlCornerRadius,
    pub top_right: ViewRuntimeControlCornerRadius,
    pub bottom_right: ViewRuntimeControlCornerRadius,
    pub bottom_left: ViewRuntimeControlCornerRadius,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlCornerRadius {
    pub x_milli: u32,
    pub y_milli: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ViewRuntimeControlRadiusDeclaration {
    Uniform(u32),
    Corners(ViewRuntimeControlRadii),
}

impl ViewRuntimeControlRadii {
    pub const fn uniform(radius_milli: u32) -> Self {
        let radius = ViewRuntimeControlCornerRadius::circular(radius_milli);
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub const fn new(
        top_left: ViewRuntimeControlCornerRadius,
        top_right: ViewRuntimeControlCornerRadius,
        bottom_right: ViewRuntimeControlCornerRadius,
        bottom_left: ViewRuntimeControlCornerRadius,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    fn is_uniform_circular(self) -> Option<u32> {
        if self.top_left.x_milli == self.top_left.y_milli
            && self.top_left == self.top_right
            && self.top_left == self.bottom_right
            && self.top_left == self.bottom_left
        {
            Some(self.top_left.x_milli)
        } else {
            None
        }
    }
}

impl ViewRuntimeControlCornerRadius {
    pub const fn circular(radius_milli: u32) -> Self {
        Self {
            x_milli: radius_milli,
            y_milli: radius_milli,
        }
    }

    pub const fn new(x_milli: u32, y_milli: u32) -> Self {
        Self { x_milli, y_milli }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeShadow {
    pub offset_x_milli: i32,
    pub offset_y_milli: i32,
    pub blur_milli: u32,
    pub spread_milli: i32,
    pub radius_milli: u32,
    pub color: RgbaColor,
    pub kind: ViewRuntimeShadowKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewRuntimeShadowKind {
    #[default]
    Outer,
    Inset,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlFilterList {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<ViewRuntimeControlFilter>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ViewRuntimeControlFilter {
    Brightness { factor_milli: u32 },
    Contrast { factor_milli: u32 },
    Grayscale { amount_milli: u16 },
    Saturate { factor_milli: u32 },
    HueRotate { degrees_milli: i32 },
    Invert { amount_milli: u16 },
    Sepia { amount_milli: u16 },
    Opacity { amount_milli: u16 },
    Blur { radius_milli: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeControlFilterSlot {
    Filter,
    BackdropFilter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewRuntimeControlState {
    Normal,
    Hover,
    Pressed,
    FocusVisible,
    Disabled,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlStyleDiagnostics {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ViewRuntimeControlStyleDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeControlStyleDiagnostic {
    pub target: String,
    pub property: String,
    pub reason: ViewRuntimeControlStyleDiagnosticReason,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewRuntimeControlStyleDiagnosticReason {
    UnsupportedProperty,
    UnsupportedValue,
    TokenNotFound,
    UnsupportedSelector,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewRuntimeControlStyleResolution {
    pub style: ViewRuntimeControlStyle,
    pub diagnostics: ViewRuntimeControlStyleDiagnostics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRuntimeStyledControls<T> {
    pub controls: Vec<T>,
    pub diagnostics: ViewRuntimeControlStyleDiagnostics,
}

impl<T> Default for ViewRuntimeStyledControls<T> {
    fn default() -> Self {
        Self {
            controls: Vec::new(),
            diagnostics: ViewRuntimeControlStyleDiagnostics::default(),
        }
    }
}

impl<T> ViewRuntimeStyledControls<T> {
    pub fn into_controls(self) -> Vec<T> {
        self.controls
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeStyleFrame {
    node: RuntimeStyleNode,
    inherited: ViewRuntimeControlStyle,
    style: ViewRuntimeControlStyle,
    binding: RuntimeStyleBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeStyleNode {
    element: Option<ViewElementKind>,
    target: String,
    part: Option<String>,
    explicit_styles: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum RuntimeStyleBinding {
    #[default]
    None,
    TextControl(usize),
    ActionButton(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeTextLeafStyle {
    node: RuntimeStyleNode,
    inherited: ViewRuntimeControlStyle,
    style: ViewRuntimeControlStyle,
    index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeStyleCombinator {
    Descendant,
    Child,
}

#[derive(Clone, Debug)]
struct RuntimeSelectorSequence<'a> {
    relationship: Option<RuntimeStyleCombinator>,
    parts: Vec<&'a ViewStyleSelectorPart>,
}

impl ViewRuntimeControlStyle {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn visual_for_state(
        &self,
        state: ViewRuntimeControlState,
    ) -> ViewRuntimeControlVisualStyle {
        let mut resolved = self.normal.clone();
        match state {
            ViewRuntimeControlState::Normal => None,
            ViewRuntimeControlState::Hover => self.hover.as_ref(),
            ViewRuntimeControlState::Pressed => self.pressed.as_ref(),
            ViewRuntimeControlState::FocusVisible => self.focus_visible.as_ref(),
            ViewRuntimeControlState::Disabled => self.disabled.as_ref(),
        }
        .into_iter()
        .for_each(|state_style| resolved.overlay(state_style));
        resolved
    }
}

impl ViewRuntimeControlVisualStyle {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    fn overlay(&mut self, patch: &Self) {
        if patch.fill.is_some() {
            self.fill = patch.fill;
        }
        if patch.text.is_some() {
            self.text = patch.text;
        }
        if patch.font_family.is_some() {
            self.font_family.clone_from(&patch.font_family);
        }
        if patch.font_size_milli.is_some() {
            self.font_size_milli = patch.font_size_milli;
        }
        if patch.line_height_milli.is_some() {
            self.line_height_milli = patch.line_height_milli;
        }
        if patch.font_weight.is_some() {
            self.font_weight = patch.font_weight;
        }
        if patch.selection.is_some() {
            self.selection = patch.selection;
        }
        if patch.caret.is_some() {
            self.caret = patch.caret;
        }
        if patch.border.is_some() {
            self.border = patch.border;
        }
        if patch.corner_frame.is_some() {
            self.corner_frame = patch.corner_frame;
        }
        if patch.focus_ring.is_some() {
            self.focus_ring = patch.focus_ring;
        }
        if patch.opacity_milli.is_some() {
            self.opacity_milli = patch.opacity_milli;
        }
        if patch.radius_milli.is_some() {
            self.radius_milli = patch.radius_milli;
            self.radii_milli = None;
        }
        if patch.radii_milli.is_some() {
            self.radii_milli = patch.radii_milli;
            self.radius_milli = None;
        }
        if patch.depth_milli.is_some() {
            self.depth_milli = patch.depth_milli;
        }
        if patch.filters.is_some() {
            self.filters.clone_from(&patch.filters);
        }
        if patch.backdrop_filters.is_some() {
            self.backdrop_filters.clone_from(&patch.backdrop_filters);
        }
        if !patch.shadows.is_empty() {
            self.shadows.clone_from(&patch.shadows);
        }
    }
}

impl ViewRuntimeControlStyleDiagnostics {
    fn push(
        &mut self,
        target: &str,
        property: impl Into<String>,
        reason: ViewRuntimeControlStyleDiagnosticReason,
    ) {
        self.diagnostics.push(ViewRuntimeControlStyleDiagnostic {
            target: target.to_owned(),
            property: property.into(),
            reason,
        });
    }

    pub fn extend(&mut self, other: Self) {
        self.diagnostics.extend(other.diagnostics);
    }

    pub const fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

impl fmt::Display for ViewRuntimeControlStyleDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "runtime control style `{}` ignored `{}`: {:?}",
            self.target, self.property, self.reason
        )
    }
}

impl SystemColor {
    pub const fn runtime_control_rgba(self) -> RgbaColor {
        match self {
            Self::Canvas => RgbaColor::rgb(5, 7, 6),
            Self::CanvasText => RgbaColor::rgb(248, 246, 234),
            Self::Panel => RgbaColor::rgb(32, 32, 25),
            Self::PanelText | Self::AccentText => RgbaColor::rgb(255, 252, 238),
            Self::RaisedPanel => RgbaColor::rgb(48, 52, 40),
            Self::MutedText => RgbaColor::rgb(172, 178, 150),
            Self::Border => RgbaColor::rgb(104, 116, 80),
            Self::Accent => RgbaColor::rgb(53, 77, 42),
            Self::FocusRing => RgbaColor::rgb(226, 233, 98),
            Self::Selection => RgbaColor::rgba(51, 153, 255, 102),
            Self::SelectionText => RgbaColor::rgb(255, 255, 255),
            Self::Danger => RgbaColor::rgb(224, 84, 84),
            Self::Warning => RgbaColor::rgb(230, 184, 92),
            Self::Success => RgbaColor::rgb(116, 198, 118),
        }
    }
}

impl ViewStyleResource {
    pub fn runtime_surface_style(&self, public_id: &str) -> ViewRuntimeControlStyleResolution {
        self.resolve_runtime_control_style(public_id, ViewElementKind::Panel, None)
    }

    fn resolve_runtime_control_style(
        &self,
        target: &str,
        element: ViewElementKind,
        explicit_style: Option<&str>,
    ) -> ViewRuntimeControlStyleResolution {
        let mut resolution = ViewRuntimeControlStyleResolution::default();
        for rule in &self.rules {
            if let Some(state) = matching_state(
                &rule.selector,
                target,
                element,
                explicit_style,
                &mut resolution,
            ) {
                apply_declarations(self, target, &mut resolution, state, &rule.declarations);
            }
        }
        for rule in self
            .part_rules
            .iter()
            .filter(|rule| part_matches(rule, target, explicit_style))
        {
            if let Some(state) = matching_state(
                &rule.selector,
                target,
                element,
                explicit_style,
                &mut resolution,
            ) {
                apply_declarations(self, target, &mut resolution, state, &rule.declarations);
            }
        }
        resolution
    }
}

impl UiInputResource {
    pub fn runtime_text_controls_with_style(
        &self,
        text: Option<&UiTextResource>,
        program: Option<&ViewProgramResource>,
        style: Option<&ViewStyleResource>,
    ) -> ViewRuntimeStyledControls<ViewRuntimeTextControl> {
        let mut diagnostics = ViewRuntimeControlStyleDiagnostics::default();
        let mut controls = self.runtime_text_controls(text, program);
        if let Some(style) = style {
            if let Some(program) = program {
                let mut action_buttons = Vec::new();
                let mut text_blocks = Vec::new();
                diagnostics.extend(program.apply_runtime_styles(
                    style,
                    &mut controls,
                    &mut action_buttons,
                    &mut text_blocks,
                ));
            }
        }
        ViewRuntimeStyledControls {
            controls,
            diagnostics,
        }
    }
}

impl ViewProgramResource {
    pub fn apply_runtime_styles(
        &self,
        style_resource: &ViewStyleResource,
        text_controls: &mut [ViewRuntimeTextControl],
        action_buttons: &mut [ViewRuntimeActionButton],
        text_blocks: &mut [ViewRuntimeTextBlock],
    ) -> ViewRuntimeControlStyleDiagnostics {
        let mut diagnostics = ViewRuntimeControlStyleDiagnostics::default();
        let mut stack = Vec::<RuntimeStyleFrame>::new();
        let mut pending_text = None::<RuntimeTextLeafStyle>;
        let mut text_control_cursor = 0_usize;
        let mut action_button_cursor = 0_usize;
        let mut text_block_cursor = 0_usize;

        for instruction in &self.instructions {
            if !matches!(instruction, ViewProgramInstruction::ApplyStyle { .. }) {
                finalize_text_leaf(&mut pending_text, text_blocks);
            }
            match instruction {
                ViewProgramInstruction::OpenElement {
                    element,
                    style,
                    part,
                    ..
                } => {
                    let inherited = stack
                        .last()
                        .map_or_else(ViewRuntimeControlStyle::default, |frame| {
                            inherited_runtime_text_style(&frame.style)
                        });
                    let binding = match element {
                        ViewElementKind::Button => {
                            next_action_button_binding(action_buttons, &mut action_button_cursor)
                        }
                        ViewElementKind::TextField
                        | ViewElementKind::TextArea
                        | ViewElementKind::SecureField => next_text_control_binding(
                            text_controls,
                            &mut text_control_cursor,
                            element.text_input_kind(),
                        ),
                        ViewElementKind::Panel
                        | ViewElementKind::Box
                        | ViewElementKind::Scroll
                        | ViewElementKind::Row
                        | ViewElementKind::Column
                        | ViewElementKind::Stack => RuntimeStyleBinding::None,
                    };
                    let target = match binding {
                        RuntimeStyleBinding::TextControl(index) => {
                            text_controls[index].public_id.clone()
                        }
                        RuntimeStyleBinding::ActionButton(index) => {
                            action_buttons[index].public_id.clone()
                        }
                        RuntimeStyleBinding::None => runtime_element_target(*element, part),
                    };
                    let mut frame = RuntimeStyleFrame {
                        node: RuntimeStyleNode {
                            element: Some(*element),
                            target,
                            part: part.clone(),
                            explicit_styles: style.iter().cloned().collect(),
                        },
                        inherited,
                        style: ViewRuntimeControlStyle::default(),
                        binding,
                    };
                    recompute_runtime_style_frame(
                        style_resource,
                        &stack,
                        &mut frame,
                        &mut diagnostics,
                    );
                    stack.push(frame);
                }
                ViewProgramInstruction::CloseElement => {
                    if let Some(frame) = stack.pop() {
                        apply_frame_binding(frame, text_controls, action_buttons);
                    }
                }
                ViewProgramInstruction::EmitText { style, part, .. } => {
                    let inherited = stack
                        .last()
                        .map_or_else(ViewRuntimeControlStyle::default, |frame| {
                            inherited_runtime_text_style(&frame.style)
                        });
                    let index = text_block_cursor;
                    text_block_cursor = text_block_cursor.saturating_add(1);
                    if index >= text_blocks.len() {
                        continue;
                    }
                    let mut leaf = RuntimeTextLeafStyle {
                        node: RuntimeStyleNode {
                            element: None,
                            target: text_blocks[index].public_id.clone(),
                            part: part.clone(),
                            explicit_styles: style.iter().cloned().collect(),
                        },
                        inherited,
                        style: ViewRuntimeControlStyle::default(),
                        index,
                    };
                    recompute_text_leaf_style(style_resource, &stack, &mut leaf, &mut diagnostics);
                    pending_text = Some(leaf);
                }
                ViewProgramInstruction::ApplyStyle { style, .. } => {
                    if let Some(leaf) = pending_text.as_mut() {
                        apply_style_ref_to_node(style_resource, style, &mut leaf.node);
                        recompute_text_leaf_style(style_resource, &stack, leaf, &mut diagnostics);
                    } else if let Some(mut frame) = stack.pop() {
                        apply_style_ref_to_node(style_resource, style, &mut frame.node);
                        recompute_runtime_style_frame(
                            style_resource,
                            &stack,
                            &mut frame,
                            &mut diagnostics,
                        );
                        stack.push(frame);
                    }
                }
                ViewProgramInstruction::EmitImage { .. }
                | ViewProgramInstruction::EmitCustom { .. }
                | ViewProgramInstruction::CallView { .. }
                | ViewProgramInstruction::Branch { .. }
                | ViewProgramInstruction::RepeatKeyed { .. }
                | ViewProgramInstruction::Await { .. }
                | ViewProgramInstruction::BindLocal { .. }
                | ViewProgramInstruction::BindHandler { .. }
                | ViewProgramInstruction::AttachSemantic { .. } => {}
            }
        }
        finalize_text_leaf(&mut pending_text, text_blocks);
        while let Some(frame) = stack.pop() {
            apply_frame_binding(frame, text_controls, action_buttons);
        }
        diagnostics
    }

    pub fn runtime_action_buttons_with_style(
        &self,
        text: Option<&UiTextResource>,
        style: Option<&ViewStyleResource>,
    ) -> ViewRuntimeStyledControls<ViewRuntimeActionButton> {
        let mut diagnostics = ViewRuntimeControlStyleDiagnostics::default();
        let mut controls = self.runtime_action_buttons(text);
        if let Some(style) = style {
            let mut text_controls = Vec::new();
            let mut text_blocks = Vec::new();
            diagnostics.extend(self.apply_runtime_styles(
                style,
                &mut text_controls,
                &mut controls,
                &mut text_blocks,
            ));
        }
        ViewRuntimeStyledControls {
            controls,
            diagnostics,
        }
    }

    pub fn runtime_text_blocks_with_style(
        &self,
        text: Option<&UiTextResource>,
        style: Option<&ViewStyleResource>,
    ) -> ViewRuntimeStyledControls<ViewRuntimeTextBlock> {
        let mut diagnostics = ViewRuntimeControlStyleDiagnostics::default();
        let mut controls = self.runtime_text_blocks(text);
        if let Some(style) = style {
            let mut text_controls = Vec::new();
            let mut action_buttons = Vec::new();
            diagnostics.extend(self.apply_runtime_styles(
                style,
                &mut text_controls,
                &mut action_buttons,
                &mut controls,
            ));
        }
        ViewRuntimeStyledControls {
            controls,
            diagnostics,
        }
    }
}

fn finalize_text_leaf(
    leaf: &mut Option<RuntimeTextLeafStyle>,
    text_blocks: &mut [ViewRuntimeTextBlock],
) {
    if let Some(leaf) = leaf.take() {
        if let Some(block) = text_blocks.get_mut(leaf.index) {
            block.style = leaf.style;
        }
    }
}

fn apply_frame_binding(
    frame: RuntimeStyleFrame,
    text_controls: &mut [ViewRuntimeTextControl],
    action_buttons: &mut [ViewRuntimeActionButton],
) {
    match frame.binding {
        RuntimeStyleBinding::None => {}
        RuntimeStyleBinding::TextControl(index) => {
            if let Some(control) = text_controls.get_mut(index) {
                control.style = frame.style;
            }
        }
        RuntimeStyleBinding::ActionButton(index) => {
            if let Some(button) = action_buttons.get_mut(index) {
                button.style = frame.style;
            }
        }
    }
}

fn next_text_control_binding(
    controls: &[ViewRuntimeTextControl],
    cursor: &mut usize,
    kind: Option<UiInputKind>,
) -> RuntimeStyleBinding {
    while *cursor < controls.len() {
        let index = *cursor;
        *cursor = (*cursor).saturating_add(1);
        if kind.is_none_or(|kind| controls[index].kind == kind) {
            return RuntimeStyleBinding::TextControl(index);
        }
    }
    RuntimeStyleBinding::None
}

fn next_action_button_binding(
    buttons: &[ViewRuntimeActionButton],
    cursor: &mut usize,
) -> RuntimeStyleBinding {
    if *cursor >= buttons.len() {
        return RuntimeStyleBinding::None;
    }
    let index = *cursor;
    *cursor = (*cursor).saturating_add(1);
    RuntimeStyleBinding::ActionButton(index)
}

fn recompute_runtime_style_frame(
    style_resource: &ViewStyleResource,
    ancestors: &[RuntimeStyleFrame],
    frame: &mut RuntimeStyleFrame,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
) {
    frame.style = frame.inherited.clone();
    apply_matching_runtime_style_rules(
        style_resource,
        ancestors,
        &frame.node,
        &mut frame.style,
        diagnostics,
    );
}

fn recompute_text_leaf_style(
    style_resource: &ViewStyleResource,
    ancestors: &[RuntimeStyleFrame],
    leaf: &mut RuntimeTextLeafStyle,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
) {
    leaf.style = leaf.inherited.clone();
    apply_matching_runtime_style_rules(
        style_resource,
        ancestors,
        &leaf.node,
        &mut leaf.style,
        diagnostics,
    );
}

fn apply_matching_runtime_style_rules(
    style_resource: &ViewStyleResource,
    ancestors: &[RuntimeStyleFrame],
    node: &RuntimeStyleNode,
    style: &mut ViewRuntimeControlStyle,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
) {
    for rule in &style_resource.rules {
        if let Some(state) =
            selector_matches_runtime_node(&rule.selector, ancestors, node, diagnostics)
        {
            apply_declarations_to_style(
                style_resource,
                &node.target,
                style,
                diagnostics,
                state,
                &rule.declarations,
            );
        }
    }
    for rule in &style_resource.part_rules {
        if !part_matches_runtime_node(&rule.part, node) {
            continue;
        }
        if let Some(state) =
            selector_matches_runtime_node(&rule.selector, ancestors, node, diagnostics)
        {
            apply_declarations_to_style(
                style_resource,
                &node.target,
                style,
                diagnostics,
                state,
                &rule.declarations,
            );
        }
    }
}

fn apply_declarations_to_style(
    style_resource: &ViewStyleResource,
    target: &str,
    style: &mut ViewRuntimeControlStyle,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    state: ViewRuntimeControlState,
    declarations: &[ViewStyleDeclaration],
) {
    let mut resolution = ViewRuntimeControlStyleResolution {
        style: core::mem::take(style),
        diagnostics: ViewRuntimeControlStyleDiagnostics::default(),
    };
    apply_declarations(style_resource, target, &mut resolution, state, declarations);
    *style = resolution.style;
    diagnostics.extend(resolution.diagnostics);
}

fn selector_matches_runtime_node(
    selector: &ViewStyleSelector,
    ancestors: &[RuntimeStyleFrame],
    node: &RuntimeStyleNode,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
) -> Option<ViewRuntimeControlState> {
    let sequences = selector_sequences(selector);
    let last_index = sequences.len().checked_sub(1)?;
    let last = &sequences[last_index];
    let state = selector_sequence_matches_node(last, node, diagnostics)?;
    let mut ancestor_limit = ancestors.len();
    for index in (0..last_index).rev() {
        let sequence = &sequences[index];
        match sequences[index + 1]
            .relationship
            .unwrap_or(RuntimeStyleCombinator::Descendant)
        {
            RuntimeStyleCombinator::Child => {
                ancestor_limit = ancestor_limit.checked_sub(1)?;
                selector_sequence_matches_node(
                    sequence,
                    &ancestors[ancestor_limit].node,
                    diagnostics,
                )?;
            }
            RuntimeStyleCombinator::Descendant => {
                let index = ancestors[..ancestor_limit].iter().rposition(|ancestor| {
                    selector_sequence_matches_node(sequence, &ancestor.node, diagnostics).is_some()
                })?;
                ancestor_limit = index;
            }
        }
    }
    Some(state)
}

fn selector_sequences(selector: &ViewStyleSelector) -> Vec<RuntimeSelectorSequence<'_>> {
    let mut sequences = vec![RuntimeSelectorSequence {
        relationship: None,
        parts: Vec::new(),
    }];
    for part in &selector.parts {
        match part {
            ViewStyleSelectorPart::Descendant => sequences.push(RuntimeSelectorSequence {
                relationship: Some(RuntimeStyleCombinator::Descendant),
                parts: Vec::new(),
            }),
            ViewStyleSelectorPart::Child => sequences.push(RuntimeSelectorSequence {
                relationship: Some(RuntimeStyleCombinator::Child),
                parts: Vec::new(),
            }),
            _ => sequences
                .last_mut()
                .expect("selector sequence exists")
                .parts
                .push(part),
        }
    }
    sequences
}

fn selector_sequence_matches_node(
    sequence: &RuntimeSelectorSequence<'_>,
    node: &RuntimeStyleNode,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
) -> Option<ViewRuntimeControlState> {
    let mut has_positive_match = sequence.parts.is_empty();
    let mut state = ViewRuntimeControlState::Normal;
    for part in &sequence.parts {
        match part {
            ViewStyleSelectorPart::Element(candidate) if node.element == Some(*candidate) => {
                has_positive_match = true;
            }
            ViewStyleSelectorPart::Part(part) if part_matches_runtime_node(part, node) => {
                has_positive_match = true;
            }
            ViewStyleSelectorPart::Element(_) | ViewStyleSelectorPart::Part(_) => return None,
            ViewStyleSelectorPart::State(ViewElementState::FocusVisible) => {
                state = ViewRuntimeControlState::FocusVisible;
            }
            ViewStyleSelectorPart::Interaction(ViewInteractionState::Hover) => {
                state = ViewRuntimeControlState::Hover;
            }
            ViewStyleSelectorPart::Interaction(ViewInteractionState::Active) => {
                state = ViewRuntimeControlState::Pressed;
            }
            ViewStyleSelectorPart::Interaction(ViewInteractionState::Disabled) => {
                state = ViewRuntimeControlState::Disabled;
            }
            ViewStyleSelectorPart::State(state) => {
                diagnostics.push(
                    &node.target,
                    format!("state::{state:?}"),
                    ViewRuntimeControlStyleDiagnosticReason::UnsupportedSelector,
                );
                return None;
            }
            ViewStyleSelectorPart::Environment(predicate) => {
                diagnostics.push(
                    &node.target,
                    format!("environment::{predicate:?}"),
                    ViewRuntimeControlStyleDiagnosticReason::UnsupportedSelector,
                );
                return None;
            }
            ViewStyleSelectorPart::Descendant | ViewStyleSelectorPart::Child => return None,
        }
    }
    has_positive_match.then_some(state)
}

fn part_matches_runtime_node(candidate: &str, node: &RuntimeStyleNode) -> bool {
    id_or_tail_matches(candidate, &node.target)
        || node
            .part
            .as_deref()
            .is_some_and(|part| id_or_tail_matches(candidate, part))
        || node
            .explicit_styles
            .iter()
            .any(|style| id_or_tail_matches(candidate, style))
}

fn apply_style_ref_to_node(
    _style_resource: &ViewStyleResource,
    style: &ViewStyleApplyRef,
    node: &mut RuntimeStyleNode,
) {
    node.explicit_styles.push(style.runtime_style_part());
}

fn inherited_runtime_text_style(style: &ViewRuntimeControlStyle) -> ViewRuntimeControlStyle {
    let source = style.visual_for_state(ViewRuntimeControlState::Normal);
    ViewRuntimeControlStyle {
        normal: ViewRuntimeControlVisualStyle {
            text: source.text,
            font_family: source.font_family,
            font_size_milli: source.font_size_milli,
            line_height_milli: source.line_height_milli,
            font_weight: source.font_weight,
            selection: source.selection,
            caret: source.caret,
            ..ViewRuntimeControlVisualStyle::default()
        },
        ..ViewRuntimeControlStyle::default()
    }
}

fn runtime_element_target(element: ViewElementKind, part: &Option<String>) -> String {
    part.clone()
        .unwrap_or_else(|| format!("element.{}", runtime_element_label(element)))
}

fn runtime_element_label(element: ViewElementKind) -> &'static str {
    match element {
        ViewElementKind::Panel => "panel",
        ViewElementKind::Box => "box",
        ViewElementKind::Scroll => "scroll",
        ViewElementKind::Row => "row",
        ViewElementKind::Column => "column",
        ViewElementKind::Stack => "stack",
        ViewElementKind::Button => "button",
        ViewElementKind::TextField => "text_field",
        ViewElementKind::TextArea => "text_area",
        ViewElementKind::SecureField => "secure_field",
    }
}

impl UiInputKind {
    pub const fn runtime_control_element(self) -> ViewElementKind {
        match self {
            Self::TextField => ViewElementKind::TextField,
            Self::TextArea => ViewElementKind::TextArea,
            Self::SecureField => ViewElementKind::SecureField,
        }
    }
}

fn part_matches(rule: &ViewPartStyleRule, target: &str, explicit_style: Option<&str>) -> bool {
    id_or_tail_matches(&rule.part, target)
        || explicit_style.is_some_and(|style| id_or_tail_matches(&rule.part, style))
}

fn id_or_tail_matches(candidate: &str, target: &str) -> bool {
    let candidate = candidate.trim().trim_start_matches('.');
    let target = target.trim().trim_start_matches('@');
    candidate == target
        || target
            .rsplit('.')
            .next()
            .is_some_and(|tail| candidate == tail)
}

fn matching_state(
    selector: &ViewStyleSelector,
    target: &str,
    element: ViewElementKind,
    explicit_style: Option<&str>,
    resolution: &mut ViewRuntimeControlStyleResolution,
) -> Option<ViewRuntimeControlState> {
    let mut has_positive_match = selector.parts.is_empty();
    let mut state = ViewRuntimeControlState::Normal;
    for part in &selector.parts {
        match part {
            ViewStyleSelectorPart::Element(candidate) if *candidate == element => {
                has_positive_match = true;
            }
            ViewStyleSelectorPart::Part(part) if part == target => has_positive_match = true,
            ViewStyleSelectorPart::Part(part) if explicit_style.is_some_and(|id| id == part) => {
                has_positive_match = true;
            }
            ViewStyleSelectorPart::Element(_) | ViewStyleSelectorPart::Part(_) => return None,
            ViewStyleSelectorPart::State(ViewElementState::FocusVisible) => {
                state = ViewRuntimeControlState::FocusVisible;
            }
            ViewStyleSelectorPart::Interaction(ViewInteractionState::Hover) => {
                state = ViewRuntimeControlState::Hover;
            }
            ViewStyleSelectorPart::Interaction(ViewInteractionState::Active) => {
                state = ViewRuntimeControlState::Pressed;
            }
            ViewStyleSelectorPart::Interaction(ViewInteractionState::Disabled) => {
                state = ViewRuntimeControlState::Disabled;
            }
            ViewStyleSelectorPart::State(state) => {
                resolution.diagnostics.push(
                    target,
                    format!("state::{state:?}"),
                    ViewRuntimeControlStyleDiagnosticReason::UnsupportedSelector,
                );
                return None;
            }
            ViewStyleSelectorPart::Environment(predicate) => {
                resolution.diagnostics.push(
                    target,
                    format!("environment::{predicate:?}"),
                    ViewRuntimeControlStyleDiagnosticReason::UnsupportedSelector,
                );
                return None;
            }
            ViewStyleSelectorPart::Descendant | ViewStyleSelectorPart::Child => {
                resolution.diagnostics.push(
                    target,
                    format!("selector::{part:?}"),
                    ViewRuntimeControlStyleDiagnosticReason::UnsupportedSelector,
                );
                return None;
            }
        }
    }
    has_positive_match.then_some(state)
}

fn apply_declarations(
    style_resource: &ViewStyleResource,
    target: &str,
    resolution: &mut ViewRuntimeControlStyleResolution,
    state: ViewRuntimeControlState,
    declarations: &[ViewStyleDeclaration],
) {
    for declaration in declarations {
        apply_declaration(style_resource, target, resolution, state, declaration);
    }
}

fn visual_slot(
    style: &mut ViewRuntimeControlStyle,
    state: ViewRuntimeControlState,
) -> &mut ViewRuntimeControlVisualStyle {
    match state {
        ViewRuntimeControlState::Normal => &mut style.normal,
        ViewRuntimeControlState::Hover => style.hover.get_or_insert_with(Default::default),
        ViewRuntimeControlState::Pressed => style.pressed.get_or_insert_with(Default::default),
        ViewRuntimeControlState::FocusVisible => {
            style.focus_visible.get_or_insert_with(Default::default)
        }
        ViewRuntimeControlState::Disabled => style.disabled.get_or_insert_with(Default::default),
    }
}

fn apply_declaration(
    style_resource: &ViewStyleResource,
    target: &str,
    resolution: &mut ViewRuntimeControlStyleResolution,
    state: ViewRuntimeControlState,
    declaration: &ViewStyleDeclaration,
) {
    let property = normalize_property(&declaration.property);
    let ViewRuntimeControlStyleResolution { style, diagnostics } = resolution;
    let visual = visual_slot(style, state);
    let value = &declaration.value;
    let raw_property = declaration.property.as_str();
    if apply_color_property_declaration(
        style_resource,
        value,
        diagnostics,
        target,
        property.as_str(),
        raw_property,
        visual,
    ) {
        return;
    }
    if apply_metric_property_declaration(
        style_resource,
        value,
        diagnostics,
        target,
        property.as_str(),
        raw_property,
        visual,
    ) {
        return;
    }
    match property.as_str() {
        "opacity" => match opacity_milli(style_resource, value) {
            Some(opacity_milli) => visual.opacity_milli = Some(opacity_milli),
            None => push_unsupported_value(diagnostics, target, raw_property),
        },
        "font-family" => match font_family_value(style_resource, value) {
            Some(font_family) => visual.font_family = Some(font_family),
            None => push_unsupported_value(diagnostics, target, raw_property),
        },
        "font-size" => match length_milli(style_resource, value) {
            Some(font_size_milli) => visual.font_size_milli = Some(font_size_milli),
            None => push_unsupported_value(diagnostics, target, raw_property),
        },
        "line-height" | "line-height-milli" => match length_milli(style_resource, value) {
            Some(line_height_milli) => visual.line_height_milli = Some(line_height_milli),
            None => push_unsupported_value(diagnostics, target, raw_property),
        },
        "font-weight" => match font_weight_value(style_resource, value) {
            Some(font_weight) => visual.font_weight = Some(font_weight),
            None => push_unsupported_value(diagnostics, target, raw_property),
        },
        "border-radius" | "radius" => match radius_declaration(style_resource, value) {
            Some(ViewRuntimeControlRadiusDeclaration::Uniform(radius_milli)) => {
                visual.radius_milli = Some(radius_milli);
                visual.radii_milli = None;
            }
            Some(ViewRuntimeControlRadiusDeclaration::Corners(radii_milli)) => {
                visual.radii_milli = Some(radii_milli);
                visual.radius_milli = None;
            }
            None => push_unsupported_value(diagnostics, target, raw_property),
        },
        "depth" | "depth-milli" | "z-index" => match depth_milli(style_resource, value) {
            Some(depth_milli) => visual.depth_milli = Some(depth_milli),
            None => push_unsupported_value(diagnostics, target, raw_property),
        },
        "filter" => apply_filter_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            RuntimeControlFilterSlot::Filter,
        ),
        "backdrop-filter" | "-webkit-backdrop-filter" => apply_filter_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            RuntimeControlFilterSlot::BackdropFilter,
        ),
        "box-shadow" => apply_shadow_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            declaration.op,
        ),
        _ => push_unsupported_property(diagnostics, target, raw_property),
    }
}

fn apply_metric_property_declaration(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    raw_property: &str,
    visual: &mut ViewRuntimeControlVisualStyle,
) -> bool {
    match property {
        "border-width" => apply_length_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, width_milli| upsert_border(visual, |border| border.width_milli = width_milli),
        ),
        "corner-frame-width" => apply_length_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, width_milli| {
                upsert_corner_frame(visual, |frame| frame.width_milli = width_milli);
            },
        ),
        "corner-frame-length" => apply_length_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, length_milli| {
                upsert_corner_frame(visual, |frame| frame.length_milli = length_milli);
            },
        ),
        "corner-frame-offset" => apply_signed_length_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, offset_milli| {
                upsert_corner_frame(visual, |frame| frame.offset_milli = offset_milli);
            },
        ),
        "focus-ring-width" | "outline-width" => apply_length_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, width_milli| {
                upsert_focus_ring(visual, |ring| ring.width_milli = width_milli);
            },
        ),
        "focus-ring-offset" | "outline-offset" => apply_signed_length_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, offset_milli| {
                upsert_focus_ring(visual, |ring| ring.offset_milli = offset_milli);
            },
        ),
        _ => return false,
    }
    true
}

fn apply_color_property_declaration(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    raw_property: &str,
    visual: &mut ViewRuntimeControlVisualStyle,
) -> bool {
    match property {
        "background" | "background-color" | "fill" => {
            apply_color_declaration(
                style_resource,
                value,
                diagnostics,
                target,
                raw_property,
                visual,
                |visual, color| visual.fill = Some(color),
            );
            true
        }
        "color" | "text-color" => {
            apply_color_declaration(
                style_resource,
                value,
                diagnostics,
                target,
                raw_property,
                visual,
                |visual, color| visual.text = Some(color),
            );
            true
        }
        "selection-color" | "selection-background-color" => {
            apply_color_declaration(
                style_resource,
                value,
                diagnostics,
                target,
                raw_property,
                visual,
                |visual, color| visual.selection = Some(color),
            );
            true
        }
        "caret-color" => {
            apply_color_declaration(
                style_resource,
                value,
                diagnostics,
                target,
                raw_property,
                visual,
                |visual, color| visual.caret = Some(color),
            );
            true
        }
        "border-color" => {
            apply_color_declaration(
                style_resource,
                value,
                diagnostics,
                target,
                raw_property,
                visual,
                |visual, color| upsert_border(visual, |border| border.color = color),
            );
            true
        }
        "corner-frame-color" => {
            apply_color_declaration(
                style_resource,
                value,
                diagnostics,
                target,
                raw_property,
                visual,
                |visual, color| upsert_corner_frame(visual, |frame| frame.color = color),
            );
            true
        }
        "focus-ring-color" | "outline-color" => {
            apply_color_declaration(
                style_resource,
                value,
                diagnostics,
                target,
                raw_property,
                visual,
                |visual, color| upsert_focus_ring(visual, |ring| ring.color = color),
            );
            true
        }
        _ => false,
    }
}

fn apply_color_declaration(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    visual: &mut ViewRuntimeControlVisualStyle,
    apply: impl FnOnce(&mut ViewRuntimeControlVisualStyle, RgbaColor),
) {
    match style_resource.color_value(value) {
        Some(color) => apply(visual, color),
        None => push_unsupported_value(diagnostics, target, property),
    }
}

fn apply_length_declaration(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    visual: &mut ViewRuntimeControlVisualStyle,
    apply: impl FnOnce(&mut ViewRuntimeControlVisualStyle, u32),
) {
    match length_milli(style_resource, value) {
        Some(length_milli) => apply(visual, length_milli),
        None => push_unsupported_value(diagnostics, target, property),
    }
}

fn apply_signed_length_declaration(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    visual: &mut ViewRuntimeControlVisualStyle,
    apply: impl FnOnce(&mut ViewRuntimeControlVisualStyle, i32),
) {
    match signed_length_milli(style_resource, value) {
        Some(length_milli) => apply(visual, length_milli),
        None => push_unsupported_value(diagnostics, target, property),
    }
}

fn apply_shadow_declaration(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    visual: &mut ViewRuntimeControlVisualStyle,
    op: StyleAssignOp,
) {
    match shadow_list(
        style_resource,
        value,
        visual.radius_milli.unwrap_or_default(),
    ) {
        Some(shadows) if op == StyleAssignOp::Append => visual.shadows.extend(shadows),
        Some(shadows) => visual.shadows = shadows,
        None => push_unsupported_value(diagnostics, target, property),
    }
}

fn apply_filter_declaration(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    visual: &mut ViewRuntimeControlVisualStyle,
    slot: RuntimeControlFilterSlot,
) {
    match filter_list(style_resource, value) {
        Some(filters) => match slot {
            RuntimeControlFilterSlot::Filter => visual.filters = Some(filters),
            RuntimeControlFilterSlot::BackdropFilter => visual.backdrop_filters = Some(filters),
        },
        None => push_unsupported_value(diagnostics, target, property),
    }
}

fn push_unsupported_value(
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
) {
    diagnostics.push(
        target,
        property,
        ViewRuntimeControlStyleDiagnosticReason::UnsupportedValue,
    );
}

fn push_unsupported_property(
    diagnostics: &mut ViewRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
) {
    diagnostics.push(
        target,
        property,
        ViewRuntimeControlStyleDiagnosticReason::UnsupportedProperty,
    );
}

trait RuntimeControlStyleValueExt {
    fn color_value(&self, value: &ViewStyleValue) -> Option<RgbaColor>;
}

impl RuntimeControlStyleValueExt for ViewStyleResource {
    fn color_value(&self, value: &ViewStyleValue) -> Option<RgbaColor> {
        match value {
            ViewStyleValue::Rgba(color) => Some(*color),
            ViewStyleValue::SystemColor(color) => Some(color.runtime_control_rgba()),
            ViewStyleValue::Token(token) => self
                .tokens
                .iter()
                .find(|candidate| candidate.public_id == *token)
                .and_then(|token| self.color_value(&token.value)),
            ViewStyleValue::Text(text) => parse_color(text),
            ViewStyleValue::Milli(_)
            | ViewStyleValue::List(_)
            | ViewStyleValue::Resource(_)
            | ViewStyleValue::Digest(_) => None,
        }
    }
}

fn normalize_property(property: &str) -> String {
    property.trim().replace('_', "-").to_ascii_lowercase()
}

fn upsert_border(
    visual: &mut ViewRuntimeControlVisualStyle,
    update: impl FnOnce(&mut ViewRuntimeControlBorderStyle),
) {
    let border = visual.border.get_or_insert(ViewRuntimeControlBorderStyle {
        color: SystemColor::Border.runtime_control_rgba(),
        width_milli: 1_000,
    });
    update(border);
}

fn upsert_corner_frame(
    visual: &mut ViewRuntimeControlVisualStyle,
    update: impl FnOnce(&mut ViewRuntimeControlCornerFrameStyle),
) {
    let corner_frame = visual
        .corner_frame
        .get_or_insert(ViewRuntimeControlCornerFrameStyle {
            color: SystemColor::FocusRing.runtime_control_rgba(),
            width_milli: 2_000,
            length_milli: 18_000,
            offset_milli: 0,
        });
    update(corner_frame);
}

fn upsert_focus_ring(
    visual: &mut ViewRuntimeControlVisualStyle,
    update: impl FnOnce(&mut ViewRuntimeControlFocusRingStyle),
) {
    let focus_ring = visual
        .focus_ring
        .get_or_insert(ViewRuntimeControlFocusRingStyle {
            color: SystemColor::FocusRing.runtime_control_rgba(),
            width_milli: 2_000,
            offset_milli: 2_000,
        });
    update(focus_ring);
}

fn length_milli(style: &ViewStyleResource, value: &ViewStyleValue) -> Option<u32> {
    signed_length_milli(style, value).and_then(|value| u32::try_from(value.max(0)).ok())
}

fn signed_length_milli(style: &ViewStyleResource, value: &ViewStyleValue) -> Option<i32> {
    match value {
        ViewStyleValue::Milli(value) => Some(*value),
        ViewStyleValue::Text(value) => parse_length_milli(value),
        ViewStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| signed_length_milli(style, &token.value)),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Resource(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn opacity_milli(style: &ViewStyleResource, value: &ViewStyleValue) -> Option<u16> {
    match value {
        ViewStyleValue::Milli(value) => u16::try_from((*value).clamp(0, 1_000)).ok(),
        ViewStyleValue::Text(value) => parse_opacity_milli(value),
        ViewStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| opacity_milli(style, &token.value)),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Resource(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn depth_milli(style: &ViewStyleResource, value: &ViewStyleValue) -> Option<i32> {
    match value {
        ViewStyleValue::Milli(value) => Some(*value),
        ViewStyleValue::Text(value) => parse_depth_milli(value),
        ViewStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| depth_milli(style, &token.value)),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Resource(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn font_family_value(style: &ViewStyleResource, value: &ViewStyleValue) -> Option<String> {
    match value {
        ViewStyleValue::Text(value) => normalized_text_value(value),
        ViewStyleValue::List(values) => {
            let values = values
                .iter()
                .map(|value| font_family_value(style, value))
                .collect::<Option<Vec<_>>>()?;
            (!values.is_empty()).then(|| values.join(", "))
        }
        ViewStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| font_family_value(style, &token.value)),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::Milli(_)
        | ViewStyleValue::Resource(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn font_weight_value(style: &ViewStyleResource, value: &ViewStyleValue) -> Option<u16> {
    match value {
        ViewStyleValue::Milli(value) => u16::try_from(*value).ok(),
        ViewStyleValue::Text(value) | ViewStyleValue::Resource(value) => parse_font_weight(value),
        ViewStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| font_weight_value(style, &token.value)),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn parse_font_weight(raw: &str) -> Option<u16> {
    match raw.trim().trim_matches('"').trim_start_matches('.') {
        "normal" | "regular" => Some(400),
        "bold" => Some(700),
        value => value.parse::<u16>().ok(),
    }
}

fn normalized_text_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn shadow_list(
    style: &ViewStyleResource,
    value: &ViewStyleValue,
    fallback_radius_milli: u32,
) -> Option<Vec<ViewRuntimeShadow>> {
    match value {
        ViewStyleValue::Text(value) => parse_shadow_list(value, fallback_radius_milli),
        ViewStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| shadow_list(style, &token.value, fallback_radius_milli)),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::Milli(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Resource(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn filter_list(
    style: &ViewStyleResource,
    value: &ViewStyleValue,
) -> Option<ViewRuntimeControlFilterList> {
    match value {
        ViewStyleValue::Text(value) => parse_filter_list(value),
        ViewStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| filter_list(style, &token.value)),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::Milli(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Resource(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn parse_opacity_milli(raw: &str) -> Option<u16> {
    let value = raw.trim();
    if let Some(percent) = value.strip_suffix('%') {
        let percent = percent.trim().parse::<f64>().ok()?;
        return rounded_clamped_i32((percent / 100.0) * 1_000.0, 0.0, 1_000.0)
            .and_then(|value| u16::try_from(value).ok());
    }
    let value = value.parse::<f64>().ok()?;
    rounded_clamped_i32(value * 1_000.0, 0.0, 1_000.0).and_then(|value| u16::try_from(value).ok())
}

fn parse_depth_milli(raw: &str) -> Option<i32> {
    raw.trim().parse::<i32>().ok()
}

fn parse_length_milli(raw: &str) -> Option<i32> {
    let value = raw.trim();
    if let Some(raw_milli) = value
        .strip_prefix("milli(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return raw_milli.trim().parse::<i32>().ok();
    }
    if let Some(raw_milli) = value.strip_suffix("milli") {
        return raw_milli.trim().parse::<i32>().ok();
    }
    let value = value.strip_suffix("px").unwrap_or(value).trim();
    let px = value.parse::<f64>().ok()?;
    rounded_clamped_i32(px * 1_000.0, f64::from(i32::MIN), f64::from(i32::MAX))
}

fn radius_declaration(
    style: &ViewStyleResource,
    value: &ViewStyleValue,
) -> Option<ViewRuntimeControlRadiusDeclaration> {
    match value {
        ViewStyleValue::Milli(value) => u32::try_from((*value).max(0))
            .ok()
            .map(ViewRuntimeControlRadiusDeclaration::Uniform),
        ViewStyleValue::Text(value) => parse_radius_declaration(value),
        ViewStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| radius_declaration(style, &token.value)),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Resource(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn parse_radius_declaration(raw: &str) -> Option<ViewRuntimeControlRadiusDeclaration> {
    let (horizontal, vertical) = raw
        .split_once('/')
        .map_or((raw, None), |(horizontal, vertical)| {
            (horizontal, Some(vertical))
        });
    let horizontal = expand_radius_values(horizontal)?;
    let vertical = vertical.map_or_else(|| Some(horizontal), expand_radius_values)?;
    let radii = ViewRuntimeControlRadii::new(
        ViewRuntimeControlCornerRadius::new(horizontal[0], vertical[0]),
        ViewRuntimeControlCornerRadius::new(horizontal[1], vertical[1]),
        ViewRuntimeControlCornerRadius::new(horizontal[2], vertical[2]),
        ViewRuntimeControlCornerRadius::new(horizontal[3], vertical[3]),
    );
    radii.is_uniform_circular().map_or_else(
        || Some(ViewRuntimeControlRadiusDeclaration::Corners(radii)),
        |radius| Some(ViewRuntimeControlRadiusDeclaration::Uniform(radius)),
    )
}

fn expand_radius_values(raw: &str) -> Option<[u32; 4]> {
    let values = raw
        .split_whitespace()
        .map(parse_non_negative_length_milli)
        .collect::<Option<Vec<_>>>()?;
    match values.as_slice() {
        [one] => Some([*one, *one, *one, *one]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top_left, horizontal, bottom_right] => {
            Some([*top_left, *horizontal, *bottom_right, *horizontal])
        }
        [top_left, top_right, bottom_right, bottom_left] => {
            Some([*top_left, *top_right, *bottom_right, *bottom_left])
        }
        _ => None,
    }
}

fn parse_non_negative_length_milli(raw: &str) -> Option<u32> {
    parse_length_milli(raw).and_then(|value| u32::try_from(value.max(0)).ok())
}

fn rounded_clamped_i32(value: f64, min: f64, max: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    format!("{:.0}", value.round().clamp(min, max))
        .parse::<i32>()
        .ok()
}

fn parse_shadow_list(raw: &str, fallback_radius_milli: u32) -> Option<Vec<ViewRuntimeShadow>> {
    let value = raw.trim();
    if value.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    split_shadow_items(value)
        .into_iter()
        .map(|item| parse_shadow_item(&item, fallback_radius_milli))
        .collect()
}

fn parse_filter_list(raw: &str) -> Option<ViewRuntimeControlFilterList> {
    let value = raw.trim();
    if value.eq_ignore_ascii_case("none") {
        return Some(ViewRuntimeControlFilterList::default());
    }

    let mut rest = value;
    let mut filters = Vec::new();
    while !rest.trim_start().is_empty() {
        let trimmed = rest.trim_start();
        let open = trimmed.find('(')?;
        let name = trimmed[..open].trim();
        let after_open = &trimmed[open + 1..];
        let close = after_open.find(')')?;
        let argument = &after_open[..close];
        filters.push(parse_filter_function(name, argument)?);
        rest = &after_open[close + 1..];
    }

    (!filters.is_empty()).then_some(ViewRuntimeControlFilterList { filters })
}

fn parse_filter_function(name: &str, argument: &str) -> Option<ViewRuntimeControlFilter> {
    match name.to_ascii_lowercase().as_str() {
        "brightness" => Some(ViewRuntimeControlFilter::Brightness {
            factor_milli: parse_filter_factor_milli(argument)?,
        }),
        "contrast" => Some(ViewRuntimeControlFilter::Contrast {
            factor_milli: parse_filter_factor_milli(argument)?,
        }),
        "grayscale" => Some(ViewRuntimeControlFilter::Grayscale {
            amount_milli: parse_filter_amount_milli(argument)?,
        }),
        "saturate" => Some(ViewRuntimeControlFilter::Saturate {
            factor_milli: parse_filter_factor_milli(argument)?,
        }),
        "hue-rotate" => Some(ViewRuntimeControlFilter::HueRotate {
            degrees_milli: parse_filter_degrees_milli(argument)?,
        }),
        "invert" => Some(ViewRuntimeControlFilter::Invert {
            amount_milli: parse_filter_amount_milli(argument)?,
        }),
        "sepia" => Some(ViewRuntimeControlFilter::Sepia {
            amount_milli: parse_filter_amount_milli(argument)?,
        }),
        "opacity" => Some(ViewRuntimeControlFilter::Opacity {
            amount_milli: parse_filter_amount_milli(argument)?,
        }),
        "blur" => Some(ViewRuntimeControlFilter::Blur {
            radius_milli: parse_filter_blur_radius_milli(argument)?,
        }),
        _ => None,
    }
}

fn parse_filter_blur_radius_milli(raw: &str) -> Option<u32> {
    let value = raw.trim();
    if value == "0" {
        return Some(0);
    }
    let px = value.strip_suffix("px")?.trim().parse::<f64>().ok()?;
    if px < 0.0 {
        return None;
    }
    rounded_clamped_i32(px * 1_000.0, 0.0, f64::from(i32::MAX))
        .and_then(|value| u32::try_from(value).ok())
}

fn parse_filter_factor_milli(raw: &str) -> Option<u32> {
    parse_filter_number_or_percent_milli(raw, 0.0, f64::from(i32::MAX))
        .and_then(|value| u32::try_from(value).ok())
}

fn parse_filter_amount_milli(raw: &str) -> Option<u16> {
    parse_filter_number_or_percent_milli(raw, 0.0, 1_000.0)
        .and_then(|value| u16::try_from(value).ok())
}

fn parse_filter_number_or_percent_milli(raw: &str, min: f64, max: f64) -> Option<i32> {
    let value = raw.trim();
    if let Some(percent) = value.strip_suffix('%') {
        let percent = percent.trim().parse::<f64>().ok()?;
        return rounded_clamped_i32((percent / 100.0) * 1_000.0, min, max);
    }
    let value = value.parse::<f64>().ok()?;
    rounded_clamped_i32(value * 1_000.0, min, max)
}

fn parse_filter_degrees_milli(raw: &str) -> Option<i32> {
    let value = raw.trim();
    if value == "0" {
        return Some(0);
    }
    let degrees = value.strip_suffix("deg")?.trim().parse::<f64>().ok()?;
    rounded_clamped_i32(degrees * 1_000.0, f64::from(i32::MIN), f64::from(i32::MAX))
}

fn split_shadow_items(value: &str) -> Vec<String> {
    let mut depth = 0_i32;
    let mut start = 0_usize;
    let mut items = Vec::new();
    for (index, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                items.push(value[start..index].trim().to_owned());
                start = index + 1;
            }
            _ => {}
        }
    }
    items.push(value[start..].trim().to_owned());
    items
}

fn parse_shadow_item(raw: &str, fallback_radius_milli: u32) -> Option<ViewRuntimeShadow> {
    let mut kind = ViewRuntimeShadowKind::Outer;
    let mut color = RgbaColor::rgba(0, 0, 0, 180);
    let mut lengths = Vec::new();
    for token in raw.split_whitespace() {
        if token.eq_ignore_ascii_case("inset") {
            kind = ViewRuntimeShadowKind::Inset;
        } else if let Some(parsed) = parse_color(token) {
            color = parsed;
        } else if let Some(length) = parse_length_milli(token) {
            lengths.push(length);
        }
    }
    let horizontal_offset_milli = *lengths.first()?;
    let vertical_offset_milli = *lengths.get(1)?;
    let blur_milli = u32::try_from(lengths.get(2).copied().unwrap_or_default().max(0)).ok()?;
    let spread_milli = lengths.get(3).copied().unwrap_or_default();
    Some(ViewRuntimeShadow {
        offset_x_milli: horizontal_offset_milli,
        offset_y_milli: vertical_offset_milli,
        blur_milli,
        spread_milli,
        radius_milli: fallback_radius_milli,
        color,
        kind,
    })
}

fn parse_color(raw: &str) -> Option<RgbaColor> {
    let value = raw.trim();
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    if let Some(args) = value
        .strip_prefix("rgba(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let mut channels = args.split(',').map(str::trim);
        let red = channels.next()?.parse::<u8>().ok()?;
        let green = channels.next()?.parse::<u8>().ok()?;
        let blue = channels.next()?.parse::<u8>().ok()?;
        let alpha = parse_alpha_channel(channels.next()?)?;
        return Some(RgbaColor::rgba(red, green, blue, alpha));
    }
    if let Some(args) = value
        .strip_prefix("rgb(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let mut channels = args.split(',').map(str::trim);
        return Some(RgbaColor::rgb(
            channels.next()?.parse::<u8>().ok()?,
            channels.next()?.parse::<u8>().ok()?,
            channels.next()?.parse::<u8>().ok()?,
        ));
    }
    None
}

fn parse_hex_color(hex: &str) -> Option<RgbaColor> {
    match hex.len() {
        6 => Some(RgbaColor::rgb(
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        )),
        8 => Some(RgbaColor::rgba(
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
        )),
        _ => None,
    }
}

fn parse_alpha_channel(raw: &str) -> Option<u8> {
    if let Some(percent) = raw.trim().strip_suffix('%') {
        let percent = percent.parse::<f64>().ok()?;
        return rounded_clamped_i32((percent / 100.0) * 255.0, 0.0, 255.0)
            .and_then(|value| u8::try_from(value).ok());
    }
    let value = raw.trim().parse::<f64>().ok()?;
    rounded_clamped_i32(value * 255.0, 0.0, 255.0).and_then(|value| u8::try_from(value).ok())
}
