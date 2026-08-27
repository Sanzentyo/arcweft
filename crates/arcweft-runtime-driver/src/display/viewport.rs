use super::{BundlePresentationUpdateError, BundleViewportFit};
use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use arcweft_layout::ScalePolicy;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PresentationViewportEffect {
    Set(BundleViewportFit),
    Clear,
}

impl PresentationViewportEffect {
    fn from_call(call: &RuntimeCall) -> Result<Option<Self>, BundlePresentationUpdateError> {
        match call.callee.as_str() {
            "player_viewport" => viewport_effect_from_call(call),
            _ => Ok(None),
        }
    }
}

pub(super) fn viewport_fit_from_effects(
    previous: Option<BundleViewportFit>,
    effects: &[LineEffectRequest],
) -> Result<Option<BundleViewportFit>, BundlePresentationUpdateError> {
    let mut active = previous;
    for effect in effects {
        let LineEffectRequest::Call(call) = effect else {
            continue;
        };
        let Some(effect) = PresentationViewportEffect::from_call(call)? else {
            continue;
        };
        active = match effect {
            PresentationViewportEffect::Set(fit) => Some(fit),
            PresentationViewportEffect::Clear => None,
        };
    }
    Ok(active)
}

fn viewport_effect_from_call(
    call: &RuntimeCall,
) -> Result<Option<PresentationViewportEffect>, BundlePresentationUpdateError> {
    let width_arg = named_arg(&call.args, "width");
    let height_arg = named_arg(&call.args, "height");
    let fit_arg = named_arg(&call.args, "fit");
    if width_arg.is_none() && height_arg.is_none() && fit_arg.is_none() {
        return Err(BundlePresentationUpdateError::missing_argument(
            "player_viewport",
            "fit or width/height",
        ));
    }
    let fit_arg = fit_arg.map_or("contain", unquote_arg);
    match fit_arg {
        "default" | "host" | "inherit" => {
            reject_viewport_dimensions_for_non_design_fit(width_arg, height_arg, fit_arg)?;
            Ok(Some(PresentationViewportEffect::Clear))
        }
        "raw" | "none" => {
            reject_viewport_dimensions_for_non_design_fit(width_arg, height_arg, fit_arg)?;
            Ok(Some(PresentationViewportEffect::Set(
                BundleViewportFit::raw(),
            )))
        }
        "contain" | "cover" | "stretch" => {
            let design_width = viewport_dimension(width_arg, "width", 1280)?;
            let design_height = viewport_dimension(height_arg, "height", 720)?;
            let scale_policy = match fit_arg {
                "cover" => ScalePolicy::Cover,
                "stretch" => ScalePolicy::Stretch,
                _ => ScalePolicy::Contain,
            };
            Ok(Some(PresentationViewportEffect::Set(
                BundleViewportFit::design(design_width, design_height, scale_policy),
            )))
        }
        value => Err(BundlePresentationUpdateError::invalid_argument(
            "player_viewport",
            "fit",
            value,
            "`raw`, `contain`, `cover`, `stretch`, or `default`",
        )),
    }
}

fn viewport_dimension(
    value: Option<&str>,
    argument: &'static str,
    default: u32,
) -> Result<u32, BundlePresentationUpdateError> {
    let Some(value) = value else {
        return Ok(default);
    };
    parse_positive_u32_px(value).ok_or_else(|| {
        BundlePresentationUpdateError::invalid_argument(
            "player_viewport",
            argument,
            value,
            "a finite positive pixel dimension",
        )
    })
}

fn reject_viewport_dimensions_for_non_design_fit(
    width: Option<&str>,
    height: Option<&str>,
    fit: &str,
) -> Result<(), BundlePresentationUpdateError> {
    let Some((argument, value)) = width
        .map(|value| ("width", value))
        .or_else(|| height.map(|value| ("height", value)))
    else {
        return Ok(());
    };
    Err(BundlePresentationUpdateError::invalid_argument(
        "player_viewport",
        argument,
        value,
        match fit {
            "raw" | "none" => "no design dimensions when fit is `raw`",
            _ => "no design dimensions when clearing the viewport override",
        },
    ))
}

fn named_arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().find_map(|arg| {
        let (arg_name, value) = arg.split_once(" = ")?;
        (arg_name.trim() == name).then_some(value.trim())
    })
}

fn parse_positive_u32_px(value: &str) -> Option<u32> {
    let value = unquote_arg(value);
    let pixels = value.strip_suffix("px").unwrap_or(value).trim();
    let parsed = pixels.parse::<f64>().ok()?.round();
    if !parsed.is_finite() || parsed < 1.0 || parsed > f64::from(u32::MAX) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(parsed as u32)
}

fn unquote_arg(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('\'')
}

#[cfg(test)]
mod tests;
