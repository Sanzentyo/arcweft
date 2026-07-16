use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, PresentationEnvironmentField, PresentationEnvironmentFieldSet,
    PresentationEnvironmentValues, TextScaleMilli,
};
use js_sys::{Array, Object, Reflect};
use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue};

const COLOR_SCHEME: &str = "colorScheme";
const CONTRAST: &str = "contrast";
const REDUCED_MOTION: &str = "reducedMotion";
const TEXT_SCALE_MILLI: &str = "textScaleMilli";

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{code}: {message}")]
pub(super) struct WebEnvironmentError {
    code: &'static str,
    message: String,
    player_id: Option<u32>,
    field: Option<String>,
}

impl WebEnvironmentError {
    pub(super) fn invalid_snapshot(message: impl Into<String>) -> Self {
        Self::new("style_environment.invalid_snapshot", message)
    }

    pub(super) fn unknown_field(field: impl Into<String>) -> Self {
        let field = field.into();
        Self::new(
            "style_environment.unknown_field",
            format!("unknown presentation environment field `{field}`"),
        )
        .with_field(field)
    }

    pub(super) fn missing_field(field: &'static str) -> Self {
        Self::new(
            "style_environment.missing_field",
            format!("missing presentation environment field `{field}`"),
        )
        .with_field(field)
    }

    pub(super) fn wrong_kind(field: &'static str, expected: &'static str) -> Self {
        Self::new(
            "style_environment.wrong_kind",
            format!("presentation environment field `{field}` must be {expected}"),
        )
        .with_field(field)
    }

    pub(super) fn text_scale_range(value: f64) -> Self {
        Self::new(
            "style_environment.text_scale_range",
            format!(
                "presentation environment field `{TEXT_SCALE_MILLI}` value {value} is outside {}..={}",
                TextScaleMilli::MIN_VALUE,
                TextScaleMilli::MAX_VALUE
            ),
        )
        .with_field(TEXT_SCALE_MILLI)
    }

    pub(super) fn reentrant_update(player_id: Option<u32>) -> Self {
        Self::new(
            "style_environment.reentrant_update",
            "presentation environment update reentered active player state",
        )
        .with_optional_player_id(player_id)
    }

    pub(super) fn player_closed(player_id: u32) -> Self {
        Self::new(
            "style_environment.player_closed",
            format!("Arcweft Web player {player_id} is closed"),
        )
        .with_player_id(player_id)
    }

    pub(super) fn unknown_player(player_id: u32) -> Self {
        Self::new(
            "style_environment.unknown_player",
            format!("Arcweft Web player {player_id} is not registry-retained"),
        )
        .with_player_id(player_id)
    }

    pub(super) fn canvas_in_use(canvas_id: &str) -> Self {
        Self::new(
            "style_environment.canvas_in_use",
            format!("canvas `{canvas_id}` already has an active Arcweft Web player"),
        )
    }

    pub(super) fn player_id_overflow() -> Self {
        Self::new(
            "style_environment.player_id_overflow",
            "Arcweft Web player identity space is exhausted",
        )
    }

    pub(super) fn revision_overflow(player_id: u32) -> Self {
        Self::new(
            "style_environment.revision_overflow",
            "presentation environment revision overflow",
        )
        .with_player_id(player_id)
    }

    pub(super) fn field_revision_overflow(
        player_id: u32,
        field: PresentationEnvironmentField,
    ) -> Self {
        let field = field_name(field);
        Self::new(
            "style_environment.field_revision_overflow",
            format!("presentation environment field revision overflow for `{field}`"),
        )
        .with_player_id(player_id)
        .with_field(field)
    }

    pub(super) fn into_js_value(self) -> JsValue {
        let object = Object::new();
        set_property(&object, "code", &JsValue::from_str(self.code));
        set_property(&object, "message", &JsValue::from_str(&self.message));
        if let Some(player_id) = self.player_id {
            set_property(
                &object,
                "playerId",
                &JsValue::from_f64(f64::from(player_id)),
            );
        }
        if let Some(field) = self.field {
            set_property(&object, "field", &JsValue::from_str(&field));
        }
        object.into()
    }

    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            player_id: None,
            field: None,
        }
    }

    fn with_player_id(self, player_id: u32) -> Self {
        self.with_optional_player_id(Some(player_id))
    }

    fn with_optional_player_id(mut self, player_id: Option<u32>) -> Self {
        self.player_id = player_id;
        self
    }

    fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
}

pub(super) fn decode_environment_snapshot(
    snapshot: &JsValue,
) -> Result<PresentationEnvironmentValues, WebEnvironmentError> {
    if !snapshot.is_object() || snapshot.is_null() || Array::is_array(snapshot) {
        return Err(WebEnvironmentError::invalid_snapshot(
            "presentation environment snapshot must be a plain non-null object",
        ));
    }
    let prototype = Reflect::get_prototype_of(snapshot).map_err(|_| {
        WebEnvironmentError::invalid_snapshot(
            "presentation environment snapshot prototype could not be inspected",
        )
    })?;
    let ordinary_prototype = Object::get_prototype_of(&Object::new().into());
    if !Object::is(prototype.as_ref(), ordinary_prototype.as_ref()) {
        return Err(WebEnvironmentError::invalid_snapshot(
            "presentation environment snapshot must have Object.prototype",
        ));
    }

    let object = snapshot.unchecked_ref::<Object>();
    let keys = Reflect::own_keys(snapshot).map_err(|_| {
        WebEnvironmentError::invalid_snapshot(
            "presentation environment snapshot properties could not be inspected",
        )
    })?;
    let mut color_scheme = None;
    let mut contrast = None;
    let mut reduced_motion = None;
    let mut text_scale = None;

    for key in keys.iter() {
        let Some(name) = key.as_string() else {
            return Err(WebEnvironmentError::unknown_field("<symbol>"));
        };
        let canonical = match name.as_str() {
            COLOR_SCHEME => COLOR_SCHEME,
            CONTRAST => CONTRAST,
            REDUCED_MOTION => REDUCED_MOTION,
            TEXT_SCALE_MILLI => TEXT_SCALE_MILLI,
            _ => return Err(WebEnvironmentError::unknown_field(name)),
        };
        let value = own_data_property(object, &key, canonical)?;
        match canonical {
            COLOR_SCHEME => {
                color_scheme = Some(match value.as_string().as_deref() {
                    Some("light") => ColorScheme::Light,
                    Some("dark") => ColorScheme::Dark,
                    _ => {
                        return Err(WebEnvironmentError::wrong_kind(
                            COLOR_SCHEME,
                            "the string `light` or `dark`",
                        ));
                    }
                });
            }
            CONTRAST => {
                contrast = Some(match value.as_string().as_deref() {
                    Some("standard") => ContrastPreference::Standard,
                    Some("more") => ContrastPreference::More,
                    _ => {
                        return Err(WebEnvironmentError::wrong_kind(
                            CONTRAST,
                            "the string `standard` or `more`",
                        ));
                    }
                });
            }
            REDUCED_MOTION => {
                reduced_motion =
                    Some(value.as_bool().ok_or_else(|| {
                        WebEnvironmentError::wrong_kind(REDUCED_MOTION, "a boolean")
                    })?);
            }
            TEXT_SCALE_MILLI => {
                let number = value.as_f64().ok_or_else(|| {
                    WebEnvironmentError::wrong_kind(TEXT_SCALE_MILLI, "an integer")
                })?;
                if !number.is_finite()
                    || number.fract() != 0.0
                    || number.abs() > 9_007_199_254_740_991.0
                {
                    return Err(WebEnvironmentError::wrong_kind(
                        TEXT_SCALE_MILLI,
                        "a finite safe integer",
                    ));
                }
                if !(f64::from(TextScaleMilli::MIN_VALUE)..=f64::from(TextScaleMilli::MAX_VALUE))
                    .contains(&number)
                {
                    return Err(WebEnvironmentError::text_scale_range(number));
                }
                text_scale = Some(
                    TextScaleMilli::try_new(number as u16)
                        .expect("the JavaScript number was checked against the canonical range"),
                );
            }
            _ => unreachable!("canonical key match is closed"),
        }
    }

    Ok(PresentationEnvironmentValues::new(
        color_scheme.ok_or_else(|| WebEnvironmentError::missing_field(COLOR_SCHEME))?,
        contrast.ok_or_else(|| WebEnvironmentError::missing_field(CONTRAST))?,
        reduced_motion.ok_or_else(|| WebEnvironmentError::missing_field(REDUCED_MOTION))?,
        text_scale.ok_or_else(|| WebEnvironmentError::missing_field(TEXT_SCALE_MILLI))?,
    ))
}

pub(super) fn environment_update_result(
    player_id: u32,
    revision: u64,
    changed_fields: PresentationEnvironmentFieldSet,
    redraw_requested: bool,
) -> JsValue {
    let object = Object::new();
    set_property(
        &object,
        "playerId",
        &JsValue::from_f64(f64::from(player_id)),
    );
    set_property(
        &object,
        "revision",
        &JsValue::from_str(&revision.to_string()),
    );
    let fields = Array::new();
    for field in changed_fields.iter() {
        fields.push(&JsValue::from_str(field_name(field)));
    }
    set_property(&object, "changedFields", fields.as_ref());
    set_property(
        &object,
        "redrawRequested",
        &JsValue::from_bool(redraw_requested),
    );
    object.into()
}

fn own_data_property(
    object: &Object,
    key: &JsValue,
    field: &'static str,
) -> Result<JsValue, WebEnvironmentError> {
    let descriptor = Reflect::get_own_property_descriptor(object, key).map_err(|_| {
        WebEnvironmentError::invalid_snapshot(format!(
            "presentation environment field `{field}` descriptor could not be inspected"
        ))
    })?;
    if descriptor.is_undefined() {
        return Err(WebEnvironmentError::missing_field(field));
    }
    let enumerable = Reflect::get(&descriptor, &JsValue::from_str("enumerable"))
        .map_err(|_| WebEnvironmentError::wrong_kind(field, "an enumerable data property"))?;
    let getter = Reflect::get(&descriptor, &JsValue::from_str("get"))
        .map_err(|_| WebEnvironmentError::wrong_kind(field, "a data property"))?;
    let setter = Reflect::get(&descriptor, &JsValue::from_str("set"))
        .map_err(|_| WebEnvironmentError::wrong_kind(field, "a data property"))?;
    if enumerable.as_bool() != Some(true) || !getter.is_undefined() || !setter.is_undefined() {
        return Err(WebEnvironmentError::wrong_kind(
            field,
            "an enumerable data property",
        ));
    }
    Reflect::get(&descriptor, &JsValue::from_str("value"))
        .map_err(|_| WebEnvironmentError::wrong_kind(field, "a readable data property"))
}

fn field_name(field: PresentationEnvironmentField) -> &'static str {
    match field {
        PresentationEnvironmentField::ColorScheme => "color_scheme",
        PresentationEnvironmentField::Contrast => "contrast",
        PresentationEnvironmentField::ReducedMotion => "reduced_motion",
        PresentationEnvironmentField::TextScale => "text_scale",
    }
}

fn set_property(object: &Object, name: &str, value: &JsValue) {
    let stored = Reflect::set(object, &JsValue::from_str(name), value)
        .expect("new plain object property assignment cannot throw");
    debug_assert!(stored);
}
