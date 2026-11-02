//! Disposable display data derived from the borrowed execution frame.
//!
//! This owner never issues a grant, stores a resource body, or replays calls.
//! Rebuilding from the complete frame removes released and displaced resources.

use crate::{
    display::BundleViewportFit,
    presentation_handles::{PresentationHandleRecord, PresentationResourceState},
};
use arcweft_core::{
    entry::RuntimeSchemaLimits,
    presentation::{
        RuntimePresentationFactoryCall, RuntimePresentationFrame, RuntimePresentationResourceRef,
    },
    value::{
        RuntimeBorrowedValue, RuntimeQuantity, RuntimeRetainedValueAdmission, RuntimeScalarView,
        RuntimeValue,
    },
};
use arcweft_id::{DeclarationIdentityFamily, presentation::PresentationHandleId};
use arcweft_interaction_model::presentation::{
    ImageObjectAlignment, ImageObjectFit, ImageObjectPlayback, ImageObjectTransform,
    PresentationArgumentKey, PresentationArgumentRole as Role, PresentationImage,
    PresentationImageBounds, PresentationImageParam, PresentationImageProxy,
    PresentationResourceKind,
};
use arcweft_layout::{
    ScalePolicy,
    stage_placement::{StageAnchor, StagePlacement, StageRect, StageSize},
};
use arcweft_primitives::LengthUnit;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PresentationProjectionError {
    #[error("presentation resource {handle} has an invalid retained factory body")]
    Body { handle: PresentationHandleId },
    #[error("presentation resource {handle} argument {role:?} requires {expected}")]
    Argument {
        handle: PresentationHandleId,
        role: Role,
        expected: &'static str,
    },
    #[error(transparent)]
    Resource(#[from] arcweft_core::value::AwbcRuntimeValueSnapshotError),
}

/// A prepared projection, committed only after every resource is admitted.
pub(crate) struct PresentationProjection {
    pub(crate) handles: Vec<PresentationHandleRecord>,
    pub(crate) images: Vec<PresentationImage>,
    pub(crate) viewport_fit: Option<BundleViewportFit>,
}

impl PresentationProjection {
    pub(crate) fn try_new(
        frame: RuntimePresentationFrame<'_>,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, PresentationProjectionError> {
        // Bound copied keys/text/actions for the entire projection before
        // allocating output tables. Capability-bearing children remain borrowed.
        let mut admission = RuntimeRetainedValueAdmission::standalone(limits);
        {
            admission.validate_metadata_collection(frame.len())?;
            for resource in frame.resources() {
                let Some(inputs) = Inputs::new(resource)? else {
                    continue;
                };
                admission.validate_metadata_collection(inputs.values.len())?;
                for ((_, key), value) in inputs.call.body_inputs().zip(inputs.values) {
                    if let Some(parameter) = key.parameter() {
                        admission.validate_metadata_string(parameter)?;
                    }
                    if let RuntimeValue::String(value) = value {
                        admission.validate_metadata_string(value)?;
                    }
                    if key.role() == Role::Actions
                        && let Some(sequence) = RuntimeBorrowedValue::from(value).sequence()
                    {
                        let count = sequence
                            .len()
                            .map_err(arcweft_core::value::AwbcRuntimeValueSnapshotError::from)?;
                        admission.validate_metadata_collection(count)?;
                        for ordinal in 0..count {
                            if let Some(RuntimeScalarView::String(value)) =
                                sequence.get(ordinal).and_then(RuntimeBorrowedValue::scalar)
                            {
                                admission.validate_metadata_string(value)?;
                            }
                        }
                    }
                }
            }
        }
        let mut projection = Self {
            handles: Vec::with_capacity(frame.len()),
            images: Vec::new(),
            viewport_fit: None,
        };
        for resource in frame.resources() {
            let inputs = Inputs::new(resource)?;
            let kind = resource.token().resource();
            let authored_visible = inputs
                .as_ref()
                .map(|inputs| inputs.boolean(Role::Visible, true))
                .transpose()?
                .unwrap_or(true);
            let visible = resource.visibility_override().unwrap_or(authored_visible);
            let resource_id = match kind {
                PresentationResourceKind::View
                | PresentationResourceKind::Menu
                | PresentationResourceKind::Overlay => {
                    if let Some(view) = resource.value().layout().view() {
                        view.view().as_str().to_owned()
                    } else {
                        let inputs = inputs.as_ref().expect("factory or View layout");
                        if let Some(RuntimeValue::PresentationResource(view)) =
                            inputs.get(Role::ViewValue)
                        {
                            view.layout()
                                .view()
                                .ok_or_else(|| {
                                    inputs.error(Role::ViewValue, "an original View value")
                                })?
                                .view()
                                .as_str()
                                .to_owned()
                        } else {
                            inputs
                                .entity(Role::View, DeclarationIdentityFamily::View)?
                                .to_owned()
                        }
                    }
                }
                PresentationResourceKind::Image => {
                    let inputs = inputs.as_ref().expect("image factory");
                    let image = inputs.image(visible)?;
                    let id = image.id.clone();
                    projection.images.push(image);
                    id
                }
                PresentationResourceKind::Background => {
                    let inputs = inputs.as_ref().expect("background factory");
                    let image = inputs.image(visible)?;
                    let id = image.id.clone();
                    projection.images.push(image);
                    id
                }
                PresentationResourceKind::Character => inputs
                    .as_ref()
                    .expect("character factory")
                    .entity(Role::Character, DeclarationIdentityFamily::Character)?
                    .to_owned(),
                PresentationResourceKind::Viewport => {
                    if visible {
                        projection.viewport_fit =
                            inputs.as_ref().expect("viewport factory").viewport()?;
                    }
                    resource.token().presentation_id().to_string()
                }
                PresentationResourceKind::RuntimeControl => {
                    return Err(PresentationProjectionError::Body {
                        handle: resource.token().presentation_id(),
                    });
                }
            };
            let inputs = inputs.as_ref();
            projection.handles.push(PresentationHandleRecord::new(
                resource.token().presentation_id(),
                kind,
                resource_id,
                inputs
                    .map(|input| input.text(Role::Owner))
                    .transpose()?
                    .flatten()
                    .map(str::to_owned),
                if visible {
                    PresentationResourceState::Mounted
                } else {
                    PresentationResourceState::Hidden
                },
                inputs
                    .map(|input| input.text(Role::LayerPublicId))
                    .transpose()?
                    .flatten()
                    .map(str::to_owned),
                inputs
                    .map(|input| input.integer(Role::Depth, 0))
                    .transpose()?
                    .unwrap_or(0),
            ));
        }
        projection.images.sort_by(|left, right| {
            (left.depth_milli, left.object_id()).cmp(&(right.depth_milli, right.object_id()))
        });
        Ok(projection)
    }
}

struct Inputs<'a> {
    resource: RuntimePresentationResourceRef<'a>,
    call: &'a RuntimePresentationFactoryCall,
    values: &'a [RuntimeValue],
}

impl<'a> Inputs<'a> {
    fn new(
        resource: RuntimePresentationResourceRef<'a>,
    ) -> Result<Option<Self>, PresentationProjectionError> {
        let Some(call) = resource.value().layout().factory() else {
            return Ok(None);
        };
        let RuntimeValue::Tuple(values) = resource.value().body() else {
            return Err(PresentationProjectionError::Body {
                handle: resource.token().presentation_id(),
            });
        };
        if call.body_len() != values.len() {
            return Err(PresentationProjectionError::Body {
                handle: resource.token().presentation_id(),
            });
        }
        Ok(Some(Self {
            resource,
            call,
            values,
        }))
    }

    fn fields(&self) -> impl Iterator<Item = (&PresentationArgumentKey, &'a RuntimeValue)> {
        self.call
            .body_inputs()
            .zip(self.values)
            .map(|((_, key), value)| (key, value))
    }

    fn get(&self, role: Role) -> Option<&'a RuntimeValue> {
        self.fields()
            .find_map(|(key, value)| (key.role() == role).then_some(value))
    }

    fn error(&self, role: Role, expected: &'static str) -> PresentationProjectionError {
        PresentationProjectionError::Argument {
            handle: self.resource.token().presentation_id(),
            role,
            expected,
        }
    }

    fn text(&self, role: Role) -> Result<Option<&'a str>, PresentationProjectionError> {
        self.get(role)
            .map(|value| match value {
                RuntimeValue::String(value) => Ok(value.as_str()),
                _ => Err(self.error(role, "String metadata")),
            })
            .transpose()
    }

    fn entity(
        &self,
        role: Role,
        family: DeclarationIdentityFamily,
    ) -> Result<&'a str, PresentationProjectionError> {
        match self.get(role) {
            Some(RuntimeValue::EntityRef(reference)) => reference
                .project_label(family)
                .ok_or_else(|| self.error(role, "the checked entity reference family")),
            _ => Err(self.error(role, "a checked entity reference")),
        }
    }

    fn boolean(&self, role: Role, default: bool) -> Result<bool, PresentationProjectionError> {
        match self.get(role) {
            None => Ok(default),
            Some(RuntimeValue::Bool(value)) => Ok(*value),
            _ => Err(self.error(role, "Bool")),
        }
    }

    fn integer(&self, role: Role, default: i32) -> Result<i32, PresentationProjectionError> {
        match self.get(role) {
            None => Ok(default),
            Some(RuntimeValue::Int(value)) => {
                value.try_into_i32().ok_or_else(|| self.error(role, "i32"))
            }
            _ => Err(self.error(role, "i32")),
        }
    }

    fn length(&self, role: Role, default: i32) -> Result<i32, PresentationProjectionError> {
        let Some(value) = self.get(role) else {
            return Ok(default);
        };
        if let Some(RuntimeQuantity::Length(length)) = RuntimeQuantity::from_value(value) {
            return if length.unit == LengthUnit::Px {
                Ok(length.milli)
            } else {
                Err(self.error(role, "a length resolved in its layout context"))
            };
        }
        let pixels = match value {
            RuntimeValue::Int(value) => {
                return value
                    .try_into_i32()
                    .and_then(|value| value.checked_mul(1_000))
                    .ok_or_else(|| self.error(role, "representable logical pixels"));
            }
            RuntimeValue::F64(value) => *value,
            RuntimeValue::String(value) => value
                .strip_suffix("px")
                .unwrap_or(value)
                .parse::<f64>()
                .map_err(|_| self.error(role, "logical pixels"))?,
            _ => return Err(self.error(role, "Length or a logical-pixel scalar")),
        };
        checked_i32(pixels * 1_000.0)
            .ok_or_else(|| self.error(role, "representable finite logical pixels"))
    }

    fn milli(&self, role: Role, default: i32) -> Result<i32, PresentationProjectionError> {
        let Some(value) = self.get(role) else {
            return Ok(default);
        };
        match RuntimeQuantity::from_value(value) {
            Some(RuntimeQuantity::Milli(value)) => return Ok(value.0),
            Some(RuntimeQuantity::Ratio(value)) => return Ok(i32::from(value.get())),
            _ => {}
        }
        let ratio = match value {
            RuntimeValue::Int(value) => {
                return value
                    .try_into_i32()
                    .ok_or_else(|| self.error(role, "i32 milli-units"));
            }
            RuntimeValue::F64(value) => *value,
            RuntimeValue::String(value) => {
                if let Some(percent) = value.strip_suffix('%') {
                    return percent
                        .parse::<f64>()
                        .ok()
                        .and_then(|value| checked_i32(value * 10.0))
                        .ok_or_else(|| self.error(role, "finite percent"));
                }
                let number = value
                    .parse::<f64>()
                    .map_err(|_| self.error(role, "a finite scalar token"))?;
                if number.abs() > 1.0 && number.fract() == 0.0 {
                    return checked_i32(number).ok_or_else(|| self.error(role, "i32 milli-units"));
                }
                number
            }
            _ => return Err(self.error(role, "a ratio or milli scalar")),
        };
        checked_i32(ratio * 1_000.0)
            .ok_or_else(|| self.error(role, "representable finite milli-units"))
    }

    fn duration(&self, role: Role) -> Result<Option<u64>, PresentationProjectionError> {
        let Some(value) = self.get(role) else {
            return Ok(None);
        };
        let nanos = match value {
            RuntimeValue::Duration(value) => return Ok(Some(value.as_nanos() / 1_000_000)),
            RuntimeValue::Int(value) => {
                return u64::try_from(value.as_i128())
                    .map(Some)
                    .map_err(|_| self.error(role, "non-negative milliseconds"));
            }
            RuntimeValue::F64(value) => *value * 1_000_000.0,
            RuntimeValue::String(value) => {
                let (number, scale) = value.strip_suffix("ms").map_or_else(
                    || {
                        value
                            .strip_suffix('s')
                            .map_or((value.as_str(), 1_000_000.0), |value| {
                                (value, 1_000_000_000.0)
                            })
                    },
                    |value| (value, 1_000_000.0),
                );
                number
                    .parse::<f64>()
                    .map_err(|_| self.error(role, "a duration token"))?
                    * scale
            }
            _ => return Err(self.error(role, "Duration or milliseconds")),
        };
        // The shared clock's nanosecond carrier is the admission boundary.
        if !nanos.is_finite() || nanos < 0.0 || nanos >= 18_446_744_073_709_551_616.0 {
            return Err(self.error(role, "representable non-negative duration"));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "finite non-negative nanoseconds checked against the exclusive u64 bound"
        )]
        Ok(Some((nanos.round_ties_even() as u64) / 1_000_000))
    }

    fn alignment(&self, role: Role) -> Result<i32, PresentationProjectionError> {
        if let Some(RuntimeValue::String(value)) = self.get(role) {
            match (role, value.as_str()) {
                (Role::AlignmentX, "left" | "start") | (Role::AlignmentY, "top" | "start") => {
                    return Ok(0);
                }
                (_, "center" | "middle") => return Ok(500),
                (Role::AlignmentX, "right" | "end") | (Role::AlignmentY, "bottom" | "end") => {
                    return Ok(1_000);
                }
                _ => {}
            }
        }
        let value = self.milli(role, 500)?;
        (0..=1_000)
            .contains(&value)
            .then_some(value)
            .ok_or_else(|| self.error(role, "alignment in 0..=1000 milli-units"))
    }

    fn image(&self, visible: bool) -> Result<PresentationImage, PresentationProjectionError> {
        let background = self.resource.token().resource() == PresentationResourceKind::Background;
        let asset = self
            .entity(Role::Asset, DeclarationIdentityFamily::Asset)?
            .to_owned();
        let occurrence = self.resource.token().presentation_id();
        let id = if background {
            // Authored slot metadata comes from the already checked address.
            let slot = self
                .resource
                .slot()
                .ok_or_else(|| self.error(Role::Slot, "an original slot address"))?;
            arcweft_presentation::BackgroundSlotAddress::try_from(slot.clone())
                .map_err(|_| self.error(Role::Slot, "an original background address"))?
                .image_id()
                .as_str()
                .to_owned()
        } else {
            self.text(Role::Id)?
                .map_or_else(|| occurrence.to_string(), str::to_owned)
        };
        let bounds = if background {
            PresentationImageBounds::try_from_px(0, 0, 1280, 720).expect("fixed stage fits")
        } else {
            let width = self.length(Role::Width, 0)?;
            let height = self.length(Role::Height, 0)?;
            PresentationImageBounds {
                x_milli: self.length(Role::X, 0)?,
                y_milli: self.length(Role::Y, 0)?,
                width_milli: u32::try_from(width)
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| self.error(Role::Width, "positive image width"))?,
                height_milli: u32::try_from(height)
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| self.error(Role::Height, "positive image height"))?,
            }
        };
        let fit = match self.text(Role::Fit)? {
            None if background => ImageObjectFit::Cover,
            None | Some("contain") => ImageObjectFit::Contain,
            Some("cover") => ImageObjectFit::Cover,
            Some("stretch") => ImageObjectFit::Stretch,
            Some("intrinsic") => ImageObjectFit::Intrinsic,
            _ => return Err(self.error(Role::Fit, "contain, cover, stretch or intrinsic")),
        };
        let opacity = self.milli(Role::Opacity, 1_000)?;
        let opacity_milli = u16::try_from(opacity)
            .ok()
            .filter(|value| *value <= 1_000)
            .ok_or_else(|| self.error(Role::Opacity, "opacity in 0..=1000 milli-units"))?;
        let rate_milli = u32::try_from(self.milli(Role::PlaybackRate, 1_000)?)
            .map_err(|_| self.error(Role::PlaybackRate, "non-negative playback rate"))?;
        let mut actions = Vec::new();
        if let Some(action) = self.text(Role::Action)? {
            actions.push(action.to_owned());
        }
        if let Some(value) = self.get(Role::Actions) {
            let sequence = RuntimeBorrowedValue::from(value)
                .sequence()
                .ok_or_else(|| self.error(Role::Actions, "a sequence of action IDs"))?;
            let count = sequence
                .len()
                .map_err(|_| self.error(Role::Actions, "a bounded sequence"))?;
            for ordinal in 0..count {
                let Some(RuntimeScalarView::String(value)) =
                    sequence.get(ordinal).and_then(RuntimeBorrowedValue::scalar)
                else {
                    return Err(self.error(Role::Actions, "String action IDs"));
                };
                actions.push(value.to_owned());
            }
        }
        let mut params = BTreeMap::new();
        let mut proxy_params = BTreeMap::new();
        for (key, value) in self
            .fields()
            .filter(|(key, _)| key.role().requires_parameter())
        {
            let parameter = key.parameter().expect("checked metadata key").to_owned();
            let value = self.parameter(key.role(), value)?;
            match key.role() {
                Role::Parameter => {
                    params.insert(parameter, value);
                }
                Role::ProxyParameter => {
                    proxy_params.insert(parameter, value);
                }
                _ => unreachable!("shared grammar owns metadata roles"),
            }
        }
        let mut proxies = Vec::new();
        if let Some(id) = self.text(Role::ProxyId)? {
            proxies.push(PresentationImageProxy {
                id: id.to_owned(),
                type_name: self.text(Role::ProxyType)?.map(str::to_owned),
                role: self.text(Role::ProxyRole)?.map(str::to_owned),
                layer: self.text(Role::ProxyLayer)?.map(str::to_owned),
                depth_milli: self
                    .get(Role::ProxyDepth)
                    .map(|_| self.integer(Role::ProxyDepth, 0))
                    .transpose()?,
                hit_test: self.boolean(Role::ProxyHitTest, false)?,
                params: proxy_params,
            });
        } else if !proxy_params.is_empty()
            || [
                Role::ProxyType,
                Role::ProxyRole,
                Role::ProxyLayer,
                Role::ProxyDepth,
                Role::ProxyHitTest,
            ]
            .into_iter()
            .any(|role| self.get(role).is_some())
        {
            return Err(self.error(Role::ProxyId, "a proxy ID for proxy metadata"));
        }
        Ok(PresentationImage {
            occurrence: Some(occurrence),
            id,
            asset,
            target: if background {
                Some(
                    self.resource
                        .slot()
                        .expect("background address admitted")
                        .target()
                        .as_str()
                        .to_owned(),
                )
            } else {
                self.text(Role::TargetPublicId)?.map(str::to_owned)
            },
            layer: if background {
                Some("layer.background".to_owned())
            } else {
                self.text(Role::LayerPublicId)?.map(str::to_owned)
            },
            view: None,
            containing_scroll_region: None,
            bounds,
            placement: Some(if background {
                StagePlacement::anchor(
                    StageAnchor::TopLeft,
                    StageAnchor::TopLeft,
                    StageSize::new(bounds.width_milli, bounds.height_milli),
                )
            } else {
                StagePlacement::absolute(StageRect::new(
                    bounds.x_milli,
                    bounds.y_milli,
                    bounds.width_milli,
                    bounds.height_milli,
                ))
            }),
            fit,
            alignment: ImageObjectAlignment::try_new(
                self.alignment(Role::AlignmentX)?,
                self.alignment(Role::AlignmentY)?,
            )
            .expect("both axes admitted"),
            playback: ImageObjectPlayback::from_parts(
                self.duration(Role::PlaybackStart)?.unwrap_or(0),
                rate_milli,
                self.duration(Role::PlaybackPausedAt)?,
                self.duration(Role::PlaybackLocalTime)?,
            ),
            transform: ImageObjectTransform {
                m11_milli: self.milli(Role::TransformM11, 1_000)?,
                m12_milli: self.milli(Role::TransformM12, 0)?,
                m21_milli: self.milli(Role::TransformM21, 0)?,
                m22_milli: self.milli(Role::TransformM22, 1_000)?,
                tx_milli: self.length(Role::TransformTx, 0)?,
                ty_milli: self.length(Role::TransformTy, 0)?,
            },
            depth_milli: self.integer(Role::Depth, 0)?,
            opacity_milli,
            actions,
            params,
            proxies,
            visible,
        })
    }

    fn parameter(
        &self,
        role: Role,
        value: &RuntimeValue,
    ) -> Result<PresentationImageParam, PresentationProjectionError> {
        if let Some(quantity) = RuntimeQuantity::from_value(value) {
            let milli = match quantity {
                RuntimeQuantity::Milli(value) => value.0,
                RuntimeQuantity::Ratio(value) => i32::from(value.get()),
                _ => return Err(self.error(role, "scalar image metadata")),
            };
            return Ok(PresentationImageParam::Milli { value: milli });
        }
        match value {
            RuntimeValue::Bool(value) => Ok(PresentationImageParam::Bool { value: *value }),
            RuntimeValue::Int(value) => value
                .try_into_i64()
                .map(|value| PresentationImageParam::Integer { value })
                .ok_or_else(|| self.error(role, "i64 image metadata")),
            RuntimeValue::String(value) => Ok(PresentationImageParam::Text {
                value: value.clone(),
            }),
            RuntimeValue::F64(value) => checked_i32(*value * 1_000.0)
                .map(|value| PresentationImageParam::Milli { value })
                .ok_or_else(|| self.error(role, "finite milli image metadata")),
            RuntimeValue::EntityRef(value) => value
                .project_family()
                .and_then(|family| value.project_label(family))
                .map(|value| PresentationImageParam::Id {
                    value: value.to_owned(),
                })
                .ok_or_else(|| self.error(role, "a checked public entity ID")),
            _ => Err(self.error(role, "scalar image metadata")),
        }
    }

    fn viewport(&self) -> Result<Option<BundleViewportFit>, PresentationProjectionError> {
        match self.text(Role::Fit)?.unwrap_or("contain") {
            "default" | "host" | "inherit" => {
                if self.get(Role::Width).is_some() || self.get(Role::Height).is_some() {
                    return Err(self.error(Role::Width, "no dimensions when clearing the viewport"));
                }
                Ok(None)
            }
            "raw" | "none" => {
                if self.get(Role::Width).is_some() || self.get(Role::Height).is_some() {
                    return Err(self.error(Role::Width, "no design dimensions for raw viewport"));
                }
                Ok(Some(BundleViewportFit::raw()))
            }
            fit @ ("contain" | "cover" | "stretch") => {
                let dimension = |role, default| {
                    let milli = self.length(role, default)?;
                    if milli <= 0 || milli % 1_000 != 0 {
                        return Err(self.error(role, "positive whole logical pixels"));
                    }
                    u32::try_from(milli / 1_000)
                        .map_err(|_| self.error(role, "representable viewport dimension"))
                };
                Ok(Some(BundleViewportFit::design(
                    dimension(Role::Width, 1_280_000)?,
                    dimension(Role::Height, 720_000)?,
                    match fit {
                        "cover" => ScalePolicy::Cover,
                        "stretch" => ScalePolicy::Stretch,
                        _ => ScalePolicy::Contain,
                    },
                )))
            }
            _ => Err(self.error(Role::Fit, "raw, contain, cover, stretch or default")),
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "ties-even rounding and both i32 bounds precede the cast"
)]
fn checked_i32(value: f64) -> Option<i32> {
    let value = value.round_ties_even();
    (value.is_finite() && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}
