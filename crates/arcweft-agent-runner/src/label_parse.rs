use std::collections::BTreeMap;

use arcweft_agent_protocol::{
    ids::PublicId,
    predicate::{CompareOp, DebugStatePath, ObservationFieldPath, Predicate, Probe},
    protocol::{
        AgentAction, AgentInvokeAction, CaptureFormat, CaptureRequest, CaptureTarget,
        ObserveRequest, PointerButton, RagRequest, WaitRequest,
    },
    value::AgentValue,
};

pub(crate) fn observe_request(args: &[String]) -> Result<ObserveRequest, String> {
    let mut request = ObserveRequest::default();
    for arg in args {
        match named_arg(arg) {
            Some(("include_images", value)) => request.include_images = parse_bool_label(value)?,
            Some(("include_objects", value)) => request.include_objects = parse_bool_label(value)?,
            Some(("include_logs", value)) => request.include_logs = parse_bool_label(value)?,
            Some((name, _)) => return Err(format!("observe has no parameter named `{name}`")),
            None => {
                return Err(format!(
                    "observe does not accept positional argument `{arg}`"
                ));
            }
        }
    }
    Ok(request)
}

pub(crate) fn capture_request(args: &[String]) -> Result<CaptureRequest, String> {
    let target = args
        .first()
        .ok_or_else(|| "capture requires a target argument".to_owned())
        .and_then(|arg| parse_capture_target(arg))?;
    let mut request = CaptureRequest {
        target,
        format: CaptureFormat::Png,
        capture_kind: "color".to_owned(),
        name: "capture".to_owned(),
    };
    for arg in args.iter().skip(1) {
        match named_arg(arg) {
            Some(("format", value)) => request.format = parse_capture_format(value)?,
            Some(("capture_kind" | "kind", value)) => {
                request.capture_kind =
                    parse_string_label(value).unwrap_or_else(|| value.to_owned());
            }
            Some(("name", value)) => {
                request.name = parse_string_label(value).unwrap_or_else(|| value.to_owned());
            }
            Some((name, _)) => return Err(format!("capture has no parameter named `{name}`")),
            None => {
                return Err(format!(
                    "capture does not accept extra positional argument `{arg}`"
                ));
            }
        }
    }
    Ok(request)
}

pub(crate) fn rag_request(args: &[String]) -> Result<RagRequest, String> {
    let query = args
        .first()
        .and_then(|arg| parse_string_label(arg))
        .ok_or_else(|| "rag.query requires a string query argument".to_owned())?;
    let mut request = RagRequest {
        query,
        roots: Vec::new(),
        graph_depth: 1,
        limit: 8,
    };
    for arg in args.iter().skip(1) {
        match named_arg(arg) {
            Some(("graph_depth", value)) => {
                request.graph_depth = value
                    .parse::<u32>()
                    .map_err(|_| format!("invalid rag.query graph_depth `{value}`"))?;
            }
            Some(("limit", value)) => {
                request.limit = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid rag.query limit `{value}`"))?;
            }
            Some(("roots", value)) => request.roots = parse_public_id_list(value)?,
            Some((name, _)) => return Err(format!("rag.query has no parameter named `{name}`")),
            None => {
                return Err(format!(
                    "rag.query does not accept extra positional argument `{arg}`"
                ));
            }
        }
    }
    Ok(request)
}

pub(crate) fn wait_request(args: &[String]) -> Result<WaitRequest, String> {
    let predicate = args
        .first()
        .ok_or_else(|| "wait requires a predicate argument".to_owned())
        .and_then(|arg| parse_predicate_label(arg))?;
    let mut request = WaitRequest {
        predicate,
        timeout_millis: 0,
        stable_frames: 1,
        poll_frames: 1,
    };
    for arg in args.iter().skip(1) {
        match named_arg(arg) {
            Some(("timeout", value)) => {
                request.timeout_millis = parse_duration_millis_label(value)?;
            }
            Some(("stable_frames", value)) => request.stable_frames = parse_u32_label(value)?,
            Some(("poll_frames", value)) => request.poll_frames = parse_u32_label(value)?,
            Some((name, _)) => return Err(format!("wait has no parameter named `{name}`")),
            None => {
                return Err(format!(
                    "wait does not accept extra positional argument `{arg}`"
                ));
            }
        }
    }
    if request.timeout_millis == 0 {
        return Err("wait requires timeout".to_owned());
    }
    Ok(request)
}

pub(crate) fn invoke_action(args: &[String]) -> Result<AgentAction, String> {
    let mut target = None;
    let mut action = None;
    let mut call_args = None;
    let mut positional = Vec::new();
    for arg in args {
        if arg.trim_start().starts_with('{') {
            positional.push(arg.as_str());
            continue;
        }
        match named_arg(arg) {
            Some(("target", value)) => target = Some(value),
            Some(("action", value)) => action = Some(value),
            Some(("args", value)) => call_args = Some(value),
            Some((name, _)) => return Err(format!("invoke has no parameter named `{name}`")),
            None => positional.push(arg.as_str()),
        }
    }
    let target = target
        .or_else(|| positional.first().copied())
        .ok_or_else(|| "invoke requires a target argument".to_owned())
        .and_then(parse_public_id_arg)?;
    let action = action
        .or_else(|| positional.get(1).copied())
        .ok_or_else(|| "invoke requires an action argument".to_owned())
        .map(parse_action_label)?;
    let call_args = call_args
        .or_else(|| positional.get(2).copied())
        .map(parse_agent_value_map_label)
        .transpose()?
        .unwrap_or_default();
    Ok(AgentAction::Invoke(Box::new(AgentInvokeAction {
        target,
        action,
        args: Box::new(call_args),
    })))
}

pub(crate) fn pointer_click_action(args: &[String]) -> Result<AgentAction, String> {
    let point = args
        .first()
        .ok_or_else(|| "pointer.click requires a point argument".to_owned())
        .and_then(|arg| parse_viewport_point_label(arg))?;
    let mut button = PointerButton::Primary;
    for arg in args.iter().skip(1) {
        match named_arg(arg) {
            Some(("button", value)) => button = parse_pointer_button_label(value)?,
            Some((name, _)) => {
                return Err(format!("pointer.click has no parameter named `{name}`"));
            }
            None => {
                return Err(format!(
                    "pointer.click does not accept extra positional argument `{arg}`"
                ));
            }
        }
    }
    Ok(AgentAction::PointerClick {
        x: point.0,
        y: point.1,
        button,
    })
}

pub(crate) fn effect_form_attachment_resource(
    args: &[String],
) -> Result<serde_json::Value, String> {
    let value = args
        .first()
        .ok_or_else(|| "attach requires a resource argument".to_owned())?;
    if args.len() > 1 {
        return Err("attach received too many positional arguments".to_owned());
    }
    Ok(parse_string_label(value).map_or_else(
        || serde_json::json!({ "label": value }),
        |value| serde_json::json!({ "label": value }),
    ))
}

fn parse_viewport_point_label(value: &str) -> Result<(u32, u32), String> {
    let value = value.trim();
    let body = value
        .strip_prefix("viewport_point(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(value);
    let parts = split_top_level_args(body)?;
    let [x, y] = parts.as_slice() else {
        return Err("viewport point requires x and y".to_owned());
    };
    Ok((parse_u32_label(x)?, parse_u32_label(y)?))
}

pub(crate) fn parse_pointer_button_label(value: &str) -> Result<PointerButton, String> {
    match value.trim().trim_start_matches('.') {
        "primary" => Ok(PointerButton::Primary),
        "secondary" => Ok(PointerButton::Secondary),
        "middle" => Ok(PointerButton::Middle),
        other => Err(format!("unsupported pointer button `{other}`")),
    }
}

fn parse_predicate_label(value: &str) -> Result<Predicate, String> {
    let value = value.trim();
    if let Some(body) = value
        .strip_prefix("exists(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return Ok(Predicate::Exists {
            probe: parse_probe_label(body)?,
        });
    }
    if let Some(body) = value
        .strip_prefix("action_enabled(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_public_id_arg(body).map(|target| Predicate::ActionEnabled { target });
    }
    if let Some(body) = value
        .strip_prefix("all(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_predicate_collection(body, false);
    }
    if let Some(body) = value
        .strip_prefix("any(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_predicate_collection(body, true);
    }
    if let Some(body) = value
        .strip_prefix("not(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_predicate_label(body).map(|predicate| Predicate::Not {
            predicate: Box::new(predicate),
        });
    }
    let (probe, method_call) = value
        .split_once(").")
        .ok_or_else(|| format!("unsupported wait predicate `{value}`"))?;
    let probe = parse_probe_label(&format!("{probe})"))?;
    let (method, expected) = method_call
        .split_once('(')
        .and_then(|(method, rest)| rest.strip_suffix(')').map(|rest| (method, rest)))
        .ok_or_else(|| format!("unsupported wait predicate `{value}`"))?;
    Ok(Predicate::Compare {
        probe,
        op: parse_compare_op_label(method)?,
        value: Box::new(parse_agent_value_label(expected)?),
    })
}

fn parse_probe_label(value: &str) -> Result<Probe, String> {
    if let Some(target) = value
        .strip_prefix("signal(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_public_id_arg(target).map(|target| Probe::Signal { target });
    }
    if let Some(target) = value
        .strip_prefix("metric(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_public_id_arg(target).map(|target| Probe::Metric { target });
    }
    if let Some(path) = value
        .strip_prefix("state(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return Ok(Probe::StatePath {
            path: DebugStatePath::new(parse_string_label(path).unwrap_or_else(|| path.to_owned()))?,
        });
    }
    if let Some(path) = value
        .strip_prefix("observation(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return Ok(Probe::ObservationField {
            path: ObservationFieldPath::new(
                parse_string_label(path).unwrap_or_else(|| path.to_owned()),
            )?,
        });
    }
    Err(format!("unsupported probe `{value}`"))
}

fn parse_compare_op_label(value: &str) -> Result<CompareOp, String> {
    match value {
        "eq" => Ok(CompareOp::Eq),
        "not_eq" | "ne" => Ok(CompareOp::NotEq),
        "gt" | "greater" => Ok(CompareOp::Greater),
        "ge" | "greater_or_equal" => Ok(CompareOp::GreaterOrEqual),
        "lt" | "less" => Ok(CompareOp::Less),
        "le" | "less_or_equal" => Ok(CompareOp::LessOrEqual),
        other => Err(format!("unsupported compare op `{other}`")),
    }
}

fn parse_agent_value_label(value: &str) -> Result<AgentValue, String> {
    match value {
        "true" => Ok(AgentValue::Bool(true)),
        "false" => Ok(AgentValue::Bool(false)),
        value if value.starts_with('@') => parse_public_id_arg(value).map(AgentValue::Entity),
        value if value.ends_with("f32") || value.ends_with("f64") => value
            .trim_end_matches("f32")
            .trim_end_matches("f64")
            .parse::<f64>()
            .map(AgentValue::F64)
            .map_err(|_| format!("invalid float literal `{value}`")),
        value if value.ends_with("u32") || value.ends_with("u64") || value.ends_with("usize") => {
            value
                .trim_end_matches("usize")
                .trim_end_matches("u32")
                .trim_end_matches("u64")
                .parse::<u64>()
                .map(AgentValue::U64)
                .map_err(|_| format!("invalid unsigned integer literal `{value}`"))
        }
        value => value.parse::<i64>().map_or_else(
            |_| {
                Ok(AgentValue::String(
                    parse_string_label(value).unwrap_or_else(|| value.to_owned()),
                ))
            },
            |value| Ok(AgentValue::I64(value)),
        ),
    }
}

fn parse_agent_value_map_label(value: &str) -> Result<BTreeMap<String, AgentValue>, String> {
    let Some(body) = value
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Err(format!("expected invoke args record, got `{value}`"));
    };
    split_top_level_args(body)?
        .into_iter()
        .map(|field| {
            record_field_arg(field)
                .ok_or_else(|| format!("expected invoke arg field, got `{field}`"))
                .and_then(|(name, value)| {
                    parse_agent_value_label(value).map(|value| (name.to_owned(), value))
                })
        })
        .collect()
}

fn parse_duration_millis_label(value: &str) -> Result<u64, String> {
    if let Some(amount) = value.strip_suffix("ms") {
        return parse_integer_millis(amount, value);
    }
    if let Some(amount) = value.strip_suffix('s') {
        return parse_seconds_millis(amount, value);
    }
    Err(format!("expected duration literal, got `{value}`"))
}

fn parse_integer_millis(amount: &str, original: &str) -> Result<u64, String> {
    if amount.contains('.') {
        return Err(format!("invalid duration literal `{original}`"));
    }
    amount
        .parse::<u64>()
        .map_err(|_| format!("invalid duration literal `{original}`"))
}

fn parse_seconds_millis(amount: &str, original: &str) -> Result<u64, String> {
    let (seconds, fraction) = amount
        .split_once('.')
        .map_or((amount, None), |(seconds, fraction)| {
            (seconds, Some(fraction))
        });
    let whole = seconds
        .parse::<u64>()
        .map_err(|_| format!("invalid duration literal `{original}`"))?;
    let millis = whole
        .checked_mul(1_000)
        .ok_or_else(|| format!("duration literal `{original}` is too large"))?;
    match fraction {
        Some(fraction)
            if !fraction.is_empty()
                && fraction.len() <= 3
                && fraction.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            let padded_fraction = format!("{fraction:0<3}");
            let fractional_millis = padded_fraction
                .parse::<u64>()
                .map_err(|_| format!("invalid duration literal `{original}`"))?;
            millis
                .checked_add(fractional_millis)
                .ok_or_else(|| format!("duration literal `{original}` is too large"))
        }
        Some(_) => Err(format!("invalid duration literal `{original}`")),
        None => Ok(millis),
    }
}

fn parse_u32_label(value: &str) -> Result<u32, String> {
    value
        .strip_suffix("u32")
        .unwrap_or(value)
        .parse::<u32>()
        .map_err(|_| format!("expected u32 literal, got `{value}`"))
}

fn parse_predicate_collection(body: &str, any: bool) -> Result<Predicate, String> {
    let predicates = split_top_level_args(body)?
        .into_iter()
        .map(parse_predicate_label)
        .collect::<Result<Vec<_>, _>>()?;
    if predicates.is_empty() {
        return Err(format!(
            "{} requires at least one predicate",
            if any { "any" } else { "all" }
        ));
    }
    Ok(if any {
        Predicate::Any { predicates }
    } else {
        Predicate::All { predicates }
    })
}

fn split_top_level_args(value: &str) -> Result<Vec<&str>, String> {
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut depth = 0u32;
    let mut in_string = false;
    let mut escape = false;
    for (index, ch) in value.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let arg = value[start..index].trim();
                if arg.is_empty() {
                    return Err("empty argument is not allowed".to_owned());
                }
                args.push(arg);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let arg = value[start..].trim();
    if arg.is_empty() && !args.is_empty() {
        return Err("empty argument is not allowed".to_owned());
    }
    if !arg.is_empty() {
        args.push(arg);
    }
    Ok(args)
}

fn named_arg(arg: &str) -> Option<(&str, &str)> {
    arg.split_once(" = ")
        .map(|(name, value)| (name.trim(), value.trim()))
}

fn record_field_arg(arg: &str) -> Option<(&str, &str)> {
    arg.split_once('=')
        .map(|(name, value)| (name.trim(), value.trim()))
        .filter(|(name, _)| !name.is_empty())
}

pub(crate) fn parse_bool_label(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("expected boolean literal, got `{value}`")),
    }
}

pub(crate) fn parse_string_label(value: &str) -> Option<String> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
}

fn parse_action_label(value: &str) -> String {
    parse_string_label(value).unwrap_or_else(|| value.strip_prefix('.').unwrap_or(value).to_owned())
}

pub(crate) fn parse_public_id_arg(value: &str) -> Result<PublicId, String> {
    let id = value.strip_prefix('@').unwrap_or(value);
    PublicId::new(id.to_owned()).map_err(|error| error.to_string())
}

pub(crate) fn parse_public_id_list(value: &str) -> Result<Vec<PublicId>, String> {
    let Some(body) = value
        .trim()
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Err(format!("expected public id list, got `{value}`"));
    };
    body.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(parse_public_id_arg)
        .collect()
}

pub(crate) fn parse_capture_target(value: &str) -> Result<CaptureTarget, String> {
    if value == "viewport()" || value == "viewport" {
        return Ok(CaptureTarget::Viewport);
    }
    if let Some(body) = call_body(value, "layer") {
        return parse_public_id_arg(body).map(|id| CaptureTarget::Layer { id });
    }
    if let Some(body) = call_body(value, "object") {
        let id =
            parse_string_label(body).unwrap_or_else(|| body.trim_start_matches('@').to_owned());
        return Ok(CaptureTarget::Object { id });
    }
    Err(format!("unsupported capture target `{value}`"))
}

fn call_body<'a>(value: &'a str, callee: &str) -> Option<&'a str> {
    value
        .strip_prefix(callee)?
        .strip_prefix('(')?
        .strip_suffix(')')
        .map(str::trim)
}

pub(crate) fn parse_capture_format(value: &str) -> Result<CaptureFormat, String> {
    match value.trim_start_matches('.') {
        "png" => Ok(CaptureFormat::Png),
        "raw_rgba" | "raw" => Ok(CaptureFormat::RawRgba),
        _ => Err(format!("unsupported capture format `{value}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_predicate_label;

    #[test]
    fn composite_predicates_reject_empty_operands() {
        for predicate in [
            "any()",
            "any(,)",
            "any(exists(signal(@signal.ready)),)",
            "all()",
            "all(exists(signal(@signal.ready)),)",
        ] {
            assert!(
                parse_predicate_label(predicate).is_err(),
                "`{predicate}` must not become a successful empty predicate"
            );
        }
    }
}
