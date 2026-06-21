#![forbid(unsafe_code)]

use std::cmp::Ordering;

use arcweft_data::{
    CodecRegistry, DataError, DataErrorKind, DecodeOptions, EncodeOptions, Result, TypeShape, Value,
};
use http::HeaderMap;
use http::header::{ACCEPT, CONTENT_TYPE};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedBody {
    pub content_type: String,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct HttpCodecOptions {
    pub decode: DecodeOptions,
    pub encode: EncodeOptions,
}

pub fn decode_request_body(
    headers: &HeaderMap,
    body: &[u8],
    shape: &TypeShape,
    registry: &CodecRegistry,
) -> Result<Value> {
    decode_request_body_with_options(headers, body, shape, registry, &HttpCodecOptions::default())
}

pub fn decode_request_body_with_options(
    headers: &HeaderMap,
    body: &[u8],
    shape: &TypeShape,
    registry: &CodecRegistry,
    options: &HttpCodecOptions,
) -> Result<Value> {
    if body.len() > options.decode.limits.max_input_len {
        return Err(DataError::limit(format!(
            "HTTP request body length {} exceeds {}",
            body.len(),
            options.decode.limits.max_input_len
        )));
    }
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| DataError::new(DataErrorKind::UnsupportedFormat, "missing Content-Type"))?;
    let media_type = parse_content_type(content_type)?;
    let codec = registry.by_media_type(&media_type)?;
    codec.decode_value(body, shape, &options.decode)
}

pub fn encode_response_body(
    headers: &HeaderMap,
    value: &Value,
    shape: &TypeShape,
    registry: &CodecRegistry,
) -> Result<EncodedBody> {
    encode_response_body_with_options(
        headers,
        value,
        shape,
        registry,
        &HttpCodecOptions::default(),
    )
}

pub fn encode_response_body_with_options(
    headers: &HeaderMap,
    value: &Value,
    shape: &TypeShape,
    registry: &CodecRegistry,
    options: &HttpCodecOptions,
) -> Result<EncodedBody> {
    let media_type = negotiate_accept(headers, registry)?;
    let codec = registry.by_media_type(&media_type)?;
    let body = codec.encode_value(value, shape, &options.encode)?;
    Ok(EncodedBody {
        content_type: media_type,
        body,
    })
}

fn parse_content_type(value: &str) -> Result<String> {
    let (media_type, params) = parse_header_item(value)?;
    if media_type.contains('*') {
        return Err(DataError::new(
            DataErrorKind::UnsupportedFormat,
            format!("Content-Type cannot be a wildcard: {value}"),
        ));
    }
    params
        .iter()
        .try_for_each(|param| parse_parameter(param).map(|_| ()))?;
    Ok(media_type)
}

fn negotiate_accept(headers: &HeaderMap, registry: &CodecRegistry) -> Result<String> {
    let values = headers
        .get_all(ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return default_response_media_type(registry);
    }

    let ranges = values
        .iter()
        .flat_map(|value| value.split(','))
        .enumerate()
        .map(|(order, item)| parse_accept_range(item, order))
        .collect::<Result<Vec<_>>>()?;
    let candidates = registered_media_types(registry);
    candidates
        .iter()
        .enumerate()
        .filter_map(|(registry_order, media_type)| {
            ranges
                .iter()
                .filter(|range| range.q > 0 && range.matches(media_type))
                .max_by(|lhs, rhs| lhs.preference_cmp(rhs))
                .map(|range| Candidate {
                    media_type: media_type.clone(),
                    q: range.q,
                    specificity: range.specificity,
                    accept_order: range.order,
                    registry_order,
                })
        })
        .max_by(Candidate::preference_cmp)
        .map(|candidate| candidate.media_type)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::UnsupportedFormat,
                format!("no registered codec satisfies Accept: {}", values.join(",")),
            )
        })
}

fn default_response_media_type(registry: &CodecRegistry) -> Result<String> {
    if registry.by_media_type("application/json").is_ok() {
        return Ok("application/json".to_owned());
    }
    registered_media_types(registry)
        .into_iter()
        .next()
        .ok_or_else(|| DataError::unsupported("no codecs are registered"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AcceptRange {
    type_: String,
    subtype: String,
    q: u16,
    specificity: u8,
    order: usize,
}

impl AcceptRange {
    fn matches(&self, media_type: &str) -> bool {
        let Some((type_, subtype)) = split_media_type(media_type) else {
            return false;
        };
        (self.type_ == "*" || self.type_ == type_)
            && (self.subtype == "*" || self.subtype == subtype)
    }

    fn preference_cmp(&self, other: &Self) -> Ordering {
        self.q
            .cmp(&other.q)
            .then_with(|| self.specificity.cmp(&other.specificity))
            .then_with(|| other.order.cmp(&self.order))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Candidate {
    media_type: String,
    q: u16,
    specificity: u8,
    accept_order: usize,
    registry_order: usize,
}

impl Candidate {
    fn preference_cmp(left: &Self, right: &Self) -> Ordering {
        left.q
            .cmp(&right.q)
            .then_with(|| left.specificity.cmp(&right.specificity))
            .then_with(|| right.accept_order.cmp(&left.accept_order))
            .then_with(|| right.registry_order.cmp(&left.registry_order))
    }
}

fn parse_accept_range(value: &str, order: usize) -> Result<AcceptRange> {
    let (media_type, params) = parse_header_item(value)?;
    let (type_, subtype) = split_media_type(&media_type).ok_or_else(|| {
        DataError::new(
            DataErrorKind::UnsupportedFormat,
            format!("invalid Accept media range `{value}`"),
        )
    })?;
    let specificity = match (type_, subtype) {
        ("*", "*") => 0,
        (_, "*") => 1,
        ("*", _) => {
            return Err(DataError::new(
                DataErrorKind::UnsupportedFormat,
                format!("invalid Accept media range `{value}`"),
            ));
        }
        _ => 2,
    };
    let q = params
        .iter()
        .map(|param| parse_parameter(param))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .find_map(|(name, value)| (name == "q").then_some(value))
        .map_or(Ok(1000), |value| parse_quality(&value))?;
    Ok(AcceptRange {
        type_: type_.to_owned(),
        subtype: subtype.to_owned(),
        q,
        specificity,
        order,
    })
}

fn parse_header_item(value: &str) -> Result<(String, Vec<&str>)> {
    let mut parts = value.split(';');
    let media_type = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
    if split_media_type(&media_type).is_none() {
        return Err(DataError::new(
            DataErrorKind::UnsupportedFormat,
            format!("invalid HTTP media type `{value}`"),
        ));
    }
    Ok((media_type, parts.collect()))
}

fn split_media_type(value: &str) -> Option<(&str, &str)> {
    let (type_, subtype) = value.split_once('/')?;
    (!type_.is_empty() && !subtype.is_empty()).then_some((type_, subtype))
}

fn parse_parameter(value: &str) -> Result<(String, String)> {
    let trimmed = value.trim();
    let (name, raw_value) = trimmed.split_once('=').ok_or_else(|| {
        DataError::new(
            DataErrorKind::UnsupportedFormat,
            format!("invalid HTTP media parameter `{trimmed}`"),
        )
    })?;
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return Err(DataError::new(
            DataErrorKind::UnsupportedFormat,
            format!("invalid HTTP media parameter `{trimmed}`"),
        ));
    }
    let value = raw_value.trim().trim_matches('"').to_owned();
    Ok((name, value))
}

fn parse_quality(value: &str) -> Result<u16> {
    let (whole, fraction) = value
        .split_once('.')
        .map_or((value, ""), |(whole, fraction)| (whole, fraction));
    match whole {
        "0" => parse_quality_fraction(fraction),
        "1" => {
            let quality = parse_quality_fraction(fraction)?;
            if quality == 0 {
                Ok(1000)
            } else {
                Err(invalid_quality(value))
            }
        }
        _ => Err(invalid_quality(value)),
    }
}

fn parse_quality_fraction(value: &str) -> Result<u16> {
    if value.is_empty() {
        return Ok(0);
    }
    if value.len() > 3 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_quality(value));
    }
    let padded = format!("{value:0<3}");
    padded.parse::<u16>().map_err(|_| invalid_quality(value))
}

fn invalid_quality(value: &str) -> DataError {
    DataError::new(
        DataErrorKind::UnsupportedFormat,
        format!("invalid Accept q value `{value}`"),
    )
}

fn registered_media_types(registry: &CodecRegistry) -> Vec<String> {
    registry
        .iter()
        .flat_map(|codec| codec.media_types())
        .map(|media_type| media_type.to_ascii_lowercase())
        .collect()
}
