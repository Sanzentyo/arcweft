use core::fmt;

use serde::{Deserialize, Serialize};

use super::model::{
    RgbaColor, StyleAssignOp, SystemColor, UiActionButtonResource, UiElementKind, UiElementState,
    UiInputKind, UiInputResource, UiInteractionState, UiPartStyleRule, UiProgramResource,
    UiRuntimeActionButton, UiRuntimeTextControl, UiStyleDeclaration, UiStyleResource,
    UiStyleSelector, UiStyleSelectorPart, UiStyleValue, UiTextResource,
};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeControlStyle {
    #[serde(
        default,
        skip_serializing_if = "UiRuntimeControlVisualStyle::is_default"
    )]
    pub normal: UiRuntimeControlVisualStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hover: Option<UiRuntimeControlVisualStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressed: Option<UiRuntimeControlVisualStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_visible: Option<UiRuntimeControlVisualStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<UiRuntimeControlVisualStyle>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeControlVisualStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<RgbaColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<RgbaColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<UiRuntimeControlBorderStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_ring: Option<UiRuntimeControlFocusRingStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity_milli: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<UiRuntimeShadow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeControlBorderStyle {
    pub color: RgbaColor,
    pub width_milli: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeControlFocusRingStyle {
    pub color: RgbaColor,
    pub width_milli: u32,
    pub offset_milli: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeShadow {
    pub offset_x_milli: i32,
    pub offset_y_milli: i32,
    pub blur_milli: u32,
    pub spread_milli: i32,
    pub radius_milli: u32,
    pub color: RgbaColor,
    pub kind: UiRuntimeShadowKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiRuntimeShadowKind {
    #[default]
    Outer,
    Inset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiRuntimeControlState {
    Normal,
    Hover,
    Pressed,
    FocusVisible,
    Disabled,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeControlStyleDiagnostics {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<UiRuntimeControlStyleDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeControlStyleDiagnostic {
    pub target: String,
    pub property: String,
    pub reason: UiRuntimeControlStyleDiagnosticReason,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiRuntimeControlStyleDiagnosticReason {
    UnsupportedProperty,
    UnsupportedValue,
    TokenNotFound,
    UnsupportedSelector,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiRuntimeControlStyleResolution {
    pub style: UiRuntimeControlStyle,
    pub diagnostics: UiRuntimeControlStyleDiagnostics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiRuntimeStyledControls<T> {
    pub controls: Vec<T>,
    pub diagnostics: UiRuntimeControlStyleDiagnostics,
}

impl<T> Default for UiRuntimeStyledControls<T> {
    fn default() -> Self {
        Self {
            controls: Vec::new(),
            diagnostics: UiRuntimeControlStyleDiagnostics::default(),
        }
    }
}

impl<T> UiRuntimeStyledControls<T> {
    pub fn into_controls(self) -> Vec<T> {
        self.controls
    }
}

impl UiRuntimeControlStyle {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn visual_for_state(&self, state: UiRuntimeControlState) -> UiRuntimeControlVisualStyle {
        let mut resolved = self.normal.clone();
        match state {
            UiRuntimeControlState::Normal => None,
            UiRuntimeControlState::Hover => self.hover.as_ref(),
            UiRuntimeControlState::Pressed => self.pressed.as_ref(),
            UiRuntimeControlState::FocusVisible => self.focus_visible.as_ref(),
            UiRuntimeControlState::Disabled => self.disabled.as_ref(),
        }
        .into_iter()
        .for_each(|state_style| resolved.overlay(state_style));
        resolved
    }
}

impl UiRuntimeControlVisualStyle {
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
        if patch.border.is_some() {
            self.border = patch.border;
        }
        if patch.focus_ring.is_some() {
            self.focus_ring = patch.focus_ring;
        }
        if patch.opacity_milli.is_some() {
            self.opacity_milli = patch.opacity_milli;
        }
        if patch.radius_milli.is_some() {
            self.radius_milli = patch.radius_milli;
        }
        if !patch.shadows.is_empty() {
            self.shadows.clone_from(&patch.shadows);
        }
    }
}

impl UiRuntimeControlStyleDiagnostics {
    fn push(
        &mut self,
        target: &str,
        property: impl Into<String>,
        reason: UiRuntimeControlStyleDiagnosticReason,
    ) {
        self.diagnostics.push(UiRuntimeControlStyleDiagnostic {
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

impl fmt::Display for UiRuntimeControlStyleDiagnostic {
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
            Self::Surface => RgbaColor::rgb(32, 32, 25),
            Self::SurfaceText | Self::AccentText => RgbaColor::rgb(255, 252, 238),
            Self::RaisedSurface => RgbaColor::rgb(48, 52, 40),
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

impl UiStyleResource {
    pub fn runtime_text_control_style(
        &self,
        public_id: &str,
        kind: UiInputKind,
    ) -> UiRuntimeControlStyleResolution {
        self.resolve_runtime_control_style(public_id, kind.runtime_control_element(), None)
    }

    pub fn runtime_action_button_style(
        &self,
        button: &UiActionButtonResource,
    ) -> UiRuntimeControlStyleResolution {
        self.resolve_runtime_control_style(
            &button.public_id,
            UiElementKind::Button,
            button.style.as_deref(),
        )
    }

    fn resolve_runtime_control_style(
        &self,
        target: &str,
        element: UiElementKind,
        explicit_style: Option<&str>,
    ) -> UiRuntimeControlStyleResolution {
        let mut resolution = UiRuntimeControlStyleResolution::default();
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
        program: Option<&UiProgramResource>,
        style: Option<&UiStyleResource>,
    ) -> UiRuntimeStyledControls<UiRuntimeTextControl> {
        let mut diagnostics = UiRuntimeControlStyleDiagnostics::default();
        let mut controls = self.runtime_text_controls(text, program);
        if let Some(style) = style {
            for control in &mut controls {
                let resolved = style.runtime_text_control_style(&control.public_id, control.kind);
                control.style = resolved.style;
                diagnostics.extend(resolved.diagnostics);
            }
        }
        UiRuntimeStyledControls {
            controls,
            diagnostics,
        }
    }
}

impl UiProgramResource {
    pub fn runtime_action_buttons_with_style(
        &self,
        text: Option<&UiTextResource>,
        style: Option<&UiStyleResource>,
    ) -> UiRuntimeStyledControls<UiRuntimeActionButton> {
        let mut diagnostics = UiRuntimeControlStyleDiagnostics::default();
        let mut controls = self.runtime_action_buttons(text);
        if let Some(style) = style {
            for control in &mut controls {
                if let Some(resource) = self
                    .action_buttons
                    .iter()
                    .find(|button| button.public_id == control.public_id)
                {
                    let resolved = style.runtime_action_button_style(resource);
                    control.style = resolved.style;
                    diagnostics.extend(resolved.diagnostics);
                }
            }
        }
        UiRuntimeStyledControls {
            controls,
            diagnostics,
        }
    }
}

impl UiInputKind {
    pub const fn runtime_control_element(self) -> UiElementKind {
        match self {
            Self::TextField => UiElementKind::TextField,
            Self::TextArea => UiElementKind::TextArea,
            Self::SecureField => UiElementKind::SecureField,
        }
    }
}

fn part_matches(rule: &UiPartStyleRule, target: &str, explicit_style: Option<&str>) -> bool {
    rule.part == target || explicit_style.is_some_and(|style| rule.part == style)
}

fn matching_state(
    selector: &UiStyleSelector,
    target: &str,
    element: UiElementKind,
    explicit_style: Option<&str>,
    resolution: &mut UiRuntimeControlStyleResolution,
) -> Option<UiRuntimeControlState> {
    let mut has_positive_match = selector.parts.is_empty();
    let mut state = UiRuntimeControlState::Normal;
    for part in &selector.parts {
        match part {
            UiStyleSelectorPart::Element(candidate) if *candidate == element => {
                has_positive_match = true;
            }
            UiStyleSelectorPart::Part(part) if part == target => has_positive_match = true,
            UiStyleSelectorPart::Part(part) if explicit_style.is_some_and(|id| id == part) => {
                has_positive_match = true;
            }
            UiStyleSelectorPart::Element(_) | UiStyleSelectorPart::Part(_) => return None,
            UiStyleSelectorPart::State(UiElementState::FocusVisible) => {
                state = UiRuntimeControlState::FocusVisible;
            }
            UiStyleSelectorPart::Interaction(UiInteractionState::Hover) => {
                state = UiRuntimeControlState::Hover;
            }
            UiStyleSelectorPart::Interaction(UiInteractionState::Active) => {
                state = UiRuntimeControlState::Pressed;
            }
            UiStyleSelectorPart::Interaction(UiInteractionState::Disabled) => {
                state = UiRuntimeControlState::Disabled;
            }
            UiStyleSelectorPart::State(state) => {
                resolution.diagnostics.push(
                    target,
                    format!("state::{state:?}"),
                    UiRuntimeControlStyleDiagnosticReason::UnsupportedSelector,
                );
                return None;
            }
            UiStyleSelectorPart::Environment(predicate) => {
                resolution.diagnostics.push(
                    target,
                    format!("environment::{predicate:?}"),
                    UiRuntimeControlStyleDiagnosticReason::UnsupportedSelector,
                );
                return None;
            }
            UiStyleSelectorPart::Descendant | UiStyleSelectorPart::Child => {
                resolution.diagnostics.push(
                    target,
                    format!("selector::{part:?}"),
                    UiRuntimeControlStyleDiagnosticReason::UnsupportedSelector,
                );
                return None;
            }
        }
    }
    has_positive_match.then_some(state)
}

fn apply_declarations(
    style_resource: &UiStyleResource,
    target: &str,
    resolution: &mut UiRuntimeControlStyleResolution,
    state: UiRuntimeControlState,
    declarations: &[UiStyleDeclaration],
) {
    for declaration in declarations {
        apply_declaration(style_resource, target, resolution, state, declaration);
    }
}

fn visual_slot(
    style: &mut UiRuntimeControlStyle,
    state: UiRuntimeControlState,
) -> &mut UiRuntimeControlVisualStyle {
    match state {
        UiRuntimeControlState::Normal => &mut style.normal,
        UiRuntimeControlState::Hover => style.hover.get_or_insert_with(Default::default),
        UiRuntimeControlState::Pressed => style.pressed.get_or_insert_with(Default::default),
        UiRuntimeControlState::FocusVisible => {
            style.focus_visible.get_or_insert_with(Default::default)
        }
        UiRuntimeControlState::Disabled => style.disabled.get_or_insert_with(Default::default),
    }
}

fn apply_declaration(
    style_resource: &UiStyleResource,
    target: &str,
    resolution: &mut UiRuntimeControlStyleResolution,
    state: UiRuntimeControlState,
    declaration: &UiStyleDeclaration,
) {
    let property = normalize_property(&declaration.property);
    let UiRuntimeControlStyleResolution { style, diagnostics } = resolution;
    let visual = visual_slot(style, state);
    let value = &declaration.value;
    let raw_property = declaration.property.as_str();
    match property.as_str() {
        "background" | "background-color" | "fill" => apply_color_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, color| visual.fill = Some(color),
        ),
        "color" | "text-color" => apply_color_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, color| visual.text = Some(color),
        ),
        "border-color" => apply_color_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, color| upsert_border(visual, |border| border.color = color),
        ),
        "border-width" => apply_length_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, width_milli| upsert_border(visual, |border| border.width_milli = width_milli),
        ),
        "focus-ring-color" | "outline-color" => apply_color_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, color| upsert_focus_ring(visual, |ring| ring.color = color),
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
        "opacity" => match opacity_milli(style_resource, value) {
            Some(opacity_milli) => visual.opacity_milli = Some(opacity_milli),
            None => push_unsupported_value(diagnostics, target, raw_property),
        },
        "border-radius" | "radius" => apply_length_declaration(
            style_resource,
            value,
            diagnostics,
            target,
            raw_property,
            visual,
            |visual, radius_milli| visual.radius_milli = Some(radius_milli),
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

fn apply_color_declaration(
    style_resource: &UiStyleResource,
    value: &UiStyleValue,
    diagnostics: &mut UiRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    visual: &mut UiRuntimeControlVisualStyle,
    apply: impl FnOnce(&mut UiRuntimeControlVisualStyle, RgbaColor),
) {
    match style_resource.color_value(value) {
        Some(color) => apply(visual, color),
        None => push_unsupported_value(diagnostics, target, property),
    }
}

fn apply_length_declaration(
    style_resource: &UiStyleResource,
    value: &UiStyleValue,
    diagnostics: &mut UiRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    visual: &mut UiRuntimeControlVisualStyle,
    apply: impl FnOnce(&mut UiRuntimeControlVisualStyle, u32),
) {
    match length_milli(style_resource, value) {
        Some(length_milli) => apply(visual, length_milli),
        None => push_unsupported_value(diagnostics, target, property),
    }
}

fn apply_signed_length_declaration(
    style_resource: &UiStyleResource,
    value: &UiStyleValue,
    diagnostics: &mut UiRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    visual: &mut UiRuntimeControlVisualStyle,
    apply: impl FnOnce(&mut UiRuntimeControlVisualStyle, i32),
) {
    match signed_length_milli(style_resource, value) {
        Some(length_milli) => apply(visual, length_milli),
        None => push_unsupported_value(diagnostics, target, property),
    }
}

fn apply_shadow_declaration(
    style_resource: &UiStyleResource,
    value: &UiStyleValue,
    diagnostics: &mut UiRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
    visual: &mut UiRuntimeControlVisualStyle,
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

fn push_unsupported_value(
    diagnostics: &mut UiRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
) {
    diagnostics.push(
        target,
        property,
        UiRuntimeControlStyleDiagnosticReason::UnsupportedValue,
    );
}

fn push_unsupported_property(
    diagnostics: &mut UiRuntimeControlStyleDiagnostics,
    target: &str,
    property: &str,
) {
    diagnostics.push(
        target,
        property,
        UiRuntimeControlStyleDiagnosticReason::UnsupportedProperty,
    );
}

trait RuntimeControlStyleValueExt {
    fn color_value(&self, value: &UiStyleValue) -> Option<RgbaColor>;
}

impl RuntimeControlStyleValueExt for UiStyleResource {
    fn color_value(&self, value: &UiStyleValue) -> Option<RgbaColor> {
        match value {
            UiStyleValue::Rgba(color) => Some(*color),
            UiStyleValue::SystemColor(color) => Some(color.runtime_control_rgba()),
            UiStyleValue::Token(token) => self
                .tokens
                .iter()
                .find(|candidate| candidate.public_id == *token)
                .and_then(|token| self.color_value(&token.value)),
            UiStyleValue::Text(text) => parse_color(text),
            UiStyleValue::Milli(_) | UiStyleValue::Resource(_) | UiStyleValue::Digest(_) => None,
        }
    }
}

fn normalize_property(property: &str) -> String {
    property.trim().replace('_', "-").to_ascii_lowercase()
}

fn upsert_border(
    visual: &mut UiRuntimeControlVisualStyle,
    update: impl FnOnce(&mut UiRuntimeControlBorderStyle),
) {
    let border = visual.border.get_or_insert(UiRuntimeControlBorderStyle {
        color: SystemColor::Border.runtime_control_rgba(),
        width_milli: 1_000,
    });
    update(border);
}

fn upsert_focus_ring(
    visual: &mut UiRuntimeControlVisualStyle,
    update: impl FnOnce(&mut UiRuntimeControlFocusRingStyle),
) {
    let focus_ring = visual
        .focus_ring
        .get_or_insert(UiRuntimeControlFocusRingStyle {
            color: SystemColor::FocusRing.runtime_control_rgba(),
            width_milli: 2_000,
            offset_milli: 2_000,
        });
    update(focus_ring);
}

fn length_milli(style: &UiStyleResource, value: &UiStyleValue) -> Option<u32> {
    signed_length_milli(style, value).and_then(|value| u32::try_from(value.max(0)).ok())
}

fn signed_length_milli(style: &UiStyleResource, value: &UiStyleValue) -> Option<i32> {
    match value {
        UiStyleValue::Milli(value) => Some(*value),
        UiStyleValue::Text(value) => parse_length_milli(value),
        UiStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| signed_length_milli(style, &token.value)),
        UiStyleValue::SystemColor(_)
        | UiStyleValue::Rgba(_)
        | UiStyleValue::Resource(_)
        | UiStyleValue::Digest(_) => None,
    }
}

fn opacity_milli(style: &UiStyleResource, value: &UiStyleValue) -> Option<u16> {
    match value {
        UiStyleValue::Milli(value) => u16::try_from((*value).clamp(0, 1_000)).ok(),
        UiStyleValue::Text(value) => parse_opacity_milli(value),
        UiStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| opacity_milli(style, &token.value)),
        UiStyleValue::SystemColor(_)
        | UiStyleValue::Rgba(_)
        | UiStyleValue::Resource(_)
        | UiStyleValue::Digest(_) => None,
    }
}

fn shadow_list(
    style: &UiStyleResource,
    value: &UiStyleValue,
    fallback_radius_milli: u32,
) -> Option<Vec<UiRuntimeShadow>> {
    match value {
        UiStyleValue::Text(value) => parse_shadow_list(value, fallback_radius_milli),
        UiStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| candidate.public_id == *token)
            .and_then(|token| shadow_list(style, &token.value, fallback_radius_milli)),
        UiStyleValue::SystemColor(_)
        | UiStyleValue::Rgba(_)
        | UiStyleValue::Milli(_)
        | UiStyleValue::Resource(_)
        | UiStyleValue::Digest(_) => None,
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

fn parse_length_milli(raw: &str) -> Option<i32> {
    let value = raw.trim().strip_suffix("px").unwrap_or(raw.trim()).trim();
    let px = value.parse::<f64>().ok()?;
    rounded_clamped_i32(px * 1_000.0, f64::from(i32::MIN), f64::from(i32::MAX))
}

fn rounded_clamped_i32(value: f64, min: f64, max: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    format!("{:.0}", value.round().clamp(min, max))
        .parse::<i32>()
        .ok()
}

fn parse_shadow_list(raw: &str, fallback_radius_milli: u32) -> Option<Vec<UiRuntimeShadow>> {
    let value = raw.trim();
    if value.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    split_shadow_items(value)
        .into_iter()
        .map(|item| parse_shadow_item(&item, fallback_radius_milli))
        .collect()
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

fn parse_shadow_item(raw: &str, fallback_radius_milli: u32) -> Option<UiRuntimeShadow> {
    let mut kind = UiRuntimeShadowKind::Outer;
    let mut color = RgbaColor::rgba(0, 0, 0, 180);
    let mut lengths = Vec::new();
    for token in raw.split_whitespace() {
        if token.eq_ignore_ascii_case("inset") {
            kind = UiRuntimeShadowKind::Inset;
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
    Some(UiRuntimeShadow {
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
