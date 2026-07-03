//! Clip-path geometry planning for the UI compositor.

use crate::ui_scene::{UiClipPath, UiFillRule, UiLength, UiPoint, UiShapeRadius};
use arcweft_presentation::hit::HitRect;
use thiserror::Error;

pub const MAX_CLIP_POLYGON_VERTICES: usize = 16;
pub const MAX_CLIP_PATH_COMMANDS: usize = 48;
pub const MAX_CLIP_PATH_EDGES: usize = 96;
const PATH_CURVE_SUBDIVISIONS: u8 = 8;
const PATH_EPSILON: f32 = 0.0001;

/// Device-independent clip geometry consumed by the analytic compositor shader.
#[derive(Clone, Debug, PartialEq)]
pub enum UiClipGeometryPlan {
    None,
    Inset {
        rect: HitRect,
        radii_px: [f32; 4],
    },
    Ellipse {
        center: UiClipVertex,
        radius_x_px: f32,
        radius_y_px: f32,
    },
    Polygon {
        fill_rule: UiFillRule,
        vertices: Vec<UiClipVertex>,
    },
    Path {
        fill_rule: UiFillRule,
        commands: Vec<UiClipPathCommandPlan>,
        edges: Vec<UiClipPathEdge>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiClipVertex {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiClipPathEdge {
    pub from: UiClipVertex,
    pub to: UiClipVertex,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiClipPathCommandPlan {
    MoveTo(UiClipVertex),
    LineTo {
        from: UiClipVertex,
        to: UiClipVertex,
    },
    QuadraticTo {
        from: UiClipVertex,
        control: UiClipVertex,
        to: UiClipVertex,
        subdivisions: u8,
    },
    CubicTo {
        from: UiClipVertex,
        control_0: UiClipVertex,
        control_1: UiClipVertex,
        to: UiClipVertex,
        subdivisions: u8,
    },
    ClosePath {
        from: UiClipVertex,
        to: UiClipVertex,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UiClipPathPlanError {
    #[error("CSS path() clip-path requires supported SVG path data commands")]
    PathUnsupported,
    #[error("clip-path url resource `{resource}` requires reusable vector clip resources")]
    UrlClipResourceUnsupported { resource: Box<str> },
    #[error("clip-path path command `{command}` is not supported by seq06.13c")]
    UnsupportedPathCommand { command: char },
    #[error("clip-path path data is malformed: {reason}")]
    MalformedPath { reason: Box<str> },
    #[error("clip-path path command `{command}` produced degenerate segment at edge {index}")]
    DegeneratePathSegment { command: char, index: usize },
    #[error("clip-path path has {count} commands but supports at most {maximum}")]
    TooManyPathCommands { count: usize, maximum: usize },
    #[error("clip-path path has {count} flattened edges but supports at most {maximum}")]
    TooManyPathEdges { count: usize, maximum: usize },
    #[error("clip-path value `{0}` has no GPU geometry lowering in seq06.13c")]
    Unsupported(Box<str>),
    #[error("clip-path length `{0}` cannot be resolved against the current bounds")]
    UnresolvableLength(Box<str>),
    #[error(
        "clip-path polygon has {count} vertices but the first shader cut supports at most {maximum}"
    )]
    TooManyPolygonVertices { count: usize, maximum: usize },
}

impl UiClipGeometryPlan {
    pub fn from_clip_path(
        clip_path: Option<&UiClipPath>,
        bounds: HitRect,
    ) -> Result<Self, UiClipPathPlanError> {
        let Some(clip_path) = clip_path else {
            return Ok(Self::None);
        };

        match clip_path {
            UiClipPath::Inset { inset, radius } => {
                let top = resolve_length(&inset[0], bounds.height, "inset-top")?;
                let right = resolve_length(&inset[1], bounds.width, "inset-right")?;
                let bottom = resolve_length(&inset[2], bounds.height, "inset-bottom")?;
                let left = resolve_length(&inset[3], bounds.width, "inset-left")?;
                let rect = HitRect::new(
                    bounds.x + left,
                    bounds.y + top,
                    (bounds.width - left - right).max(0.0),
                    (bounds.height - top - bottom).max(0.0),
                );
                let radii_px = [
                    resolve_length(
                        &radius[0],
                        bounds.width.min(bounds.height),
                        "radius-top-left",
                    )?,
                    resolve_length(
                        &radius[1],
                        bounds.width.min(bounds.height),
                        "radius-top-right",
                    )?,
                    resolve_length(
                        &radius[2],
                        bounds.width.min(bounds.height),
                        "radius-bottom-right",
                    )?,
                    resolve_length(
                        &radius[3],
                        bounds.width.min(bounds.height),
                        "radius-bottom-left",
                    )?,
                ];
                Ok(Self::Inset { rect, radii_px })
            }
            UiClipPath::Circle { radius, center } => {
                let center = resolve_point(center, bounds)?;
                let radius_px = resolve_shape_radius(radius, bounds, true)?;
                Ok(Self::Ellipse {
                    center,
                    radius_x_px: radius_px,
                    radius_y_px: radius_px,
                })
            }
            UiClipPath::Ellipse {
                radius_x,
                radius_y,
                center,
            } => Ok(Self::Ellipse {
                center: resolve_point(center, bounds)?,
                radius_x_px: resolve_shape_radius(radius_x, bounds, true)?,
                radius_y_px: resolve_shape_radius(radius_y, bounds, false)?,
            }),
            UiClipPath::Polygon { fill_rule, points } => {
                if points.len() > MAX_CLIP_POLYGON_VERTICES {
                    return Err(UiClipPathPlanError::TooManyPolygonVertices {
                        count: points.len(),
                        maximum: MAX_CLIP_POLYGON_VERTICES,
                    });
                }
                Ok(Self::Polygon {
                    fill_rule: *fill_rule,
                    vertices: points
                        .iter()
                        .map(|point| resolve_point(point, bounds))
                        .collect::<Result<Vec<_>, _>>()?,
                })
            }
            UiClipPath::Path { fill_rule, data } => path_plan(*fill_rule, data, bounds),
            UiClipPath::Url(resource) => Err(UiClipPathPlanError::UrlClipResourceUnsupported {
                resource: resource.clone(),
            }),
            UiClipPath::Unsupported(reason) => {
                Err(UiClipPathPlanError::Unsupported(reason.clone()))
            }
        }
    }

    pub fn requires_geometry_pass(&self) -> bool {
        !matches!(self, Self::None)
    }
}

fn resolve_shape_radius(
    radius: &UiShapeRadius,
    bounds: HitRect,
    horizontal: bool,
) -> Result<f32, UiClipPathPlanError> {
    Ok(match radius {
        UiShapeRadius::ClosestSide => bounds.width.min(bounds.height) * 0.5,
        UiShapeRadius::FarthestSide => bounds.width.max(bounds.height) * 0.5,
        UiShapeRadius::Length(length) => {
            let basis = if horizontal {
                bounds.width
            } else {
                bounds.height
            };
            resolve_length(length, basis, "shape-radius")?
        }
    }
    .max(0.0))
}

fn resolve_point(point: &UiPoint, bounds: HitRect) -> Result<UiClipVertex, UiClipPathPlanError> {
    Ok(UiClipVertex {
        x: bounds.x + resolve_length(&point.x, bounds.width, "point-x")?,
        y: bounds.y + resolve_length(&point.y, bounds.height, "point-y")?,
    })
}

fn resolve_length(
    length: &UiLength,
    basis_px: f32,
    role: &'static str,
) -> Result<f32, UiClipPathPlanError> {
    match length {
        UiLength::Px(value) => Ok(*value),
        UiLength::Percent(value) => Ok(*value * basis_px),
        UiLength::Auto => Err(UiClipPathPlanError::UnresolvableLength(role.into())),
        UiLength::Unsupported(reason) => Err(UiClipPathPlanError::Unsupported(reason.clone())),
    }
}

fn path_plan(
    fill_rule: UiFillRule,
    data: &str,
    bounds: HitRect,
) -> Result<UiClipGeometryPlan, UiClipPathPlanError> {
    let tokens = tokenize_path(data)?;
    let mut parser = PathParser::new(tokens, bounds);
    let mut path = ParsedPath::default();
    parser.parse(&mut path)?;
    if path.edges.is_empty() {
        return Err(UiClipPathPlanError::MalformedPath {
            reason: "path has no drawable segments".into(),
        });
    }
    Ok(UiClipGeometryPlan::Path {
        fill_rule,
        commands: path.commands,
        edges: path.edges,
    })
}

#[derive(Default)]
struct ParsedPath {
    commands: Vec<UiClipPathCommandPlan>,
    edges: Vec<UiClipPathEdge>,
}

impl ParsedPath {
    fn push_command(&mut self, command: UiClipPathCommandPlan) -> Result<(), UiClipPathPlanError> {
        let count = self.commands.len() + 1;
        if count > MAX_CLIP_PATH_COMMANDS {
            return Err(UiClipPathPlanError::TooManyPathCommands {
                count,
                maximum: MAX_CLIP_PATH_COMMANDS,
            });
        }
        self.commands.push(command);
        Ok(())
    }

    fn push_edge(
        &mut self,
        command: char,
        from: UiClipVertex,
        to: UiClipVertex,
    ) -> Result<(), UiClipPathPlanError> {
        let index = self.edges.len();
        if !from.x.is_finite() || !from.y.is_finite() || !to.x.is_finite() || !to.y.is_finite() {
            return Err(UiClipPathPlanError::DegeneratePathSegment { command, index });
        }
        if distance_squared(from, to) <= PATH_EPSILON * PATH_EPSILON {
            return Err(UiClipPathPlanError::DegeneratePathSegment { command, index });
        }
        let count = index + 1;
        if count > MAX_CLIP_PATH_EDGES {
            return Err(UiClipPathPlanError::TooManyPathEdges {
                count,
                maximum: MAX_CLIP_PATH_EDGES,
            });
        }
        self.edges.push(UiClipPathEdge { from, to });
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PathToken {
    Command(char),
    Number(f32),
}

struct PathParser {
    tokens: Vec<PathToken>,
    index: usize,
    bounds: HitRect,
    current: UiClipVertex,
    subpath_start: Option<UiClipVertex>,
    last_command: Option<char>,
}

impl PathParser {
    fn new(tokens: Vec<PathToken>, bounds: HitRect) -> Self {
        Self {
            tokens,
            index: 0,
            bounds,
            current: UiClipVertex {
                x: bounds.x,
                y: bounds.y,
            },
            subpath_start: None,
            last_command: None,
        }
    }

    fn parse(&mut self, path: &mut ParsedPath) -> Result<(), UiClipPathPlanError> {
        while self.index < self.tokens.len() {
            let command = match self.peek().copied() {
                Some(PathToken::Command(command)) => {
                    self.index += 1;
                    command
                }
                Some(PathToken::Number(_)) => {
                    self.last_command
                        .ok_or_else(|| UiClipPathPlanError::MalformedPath {
                            reason: "path data starts with a number before a command".into(),
                        })?
                }
                None => break,
            };
            self.run_command(command, path)?;
            self.last_command = Some(implicit_repeat_command(command));
        }
        Ok(())
    }

    fn run_command(
        &mut self,
        command: char,
        path: &mut ParsedPath,
    ) -> Result<(), UiClipPathPlanError> {
        match command {
            'M' | 'm' => self.move_to(command, path),
            'L' | 'l' => self.line_to(command, path),
            'H' | 'h' => self.horizontal_line_to(command, path),
            'V' | 'v' => self.vertical_line_to(command, path),
            'Q' | 'q' => self.quadratic_to(command, path),
            'C' | 'c' => self.cubic_to(command, path),
            'Z' | 'z' => self.close_path(command, path),
            other if other.is_ascii_alphabetic() => {
                Err(UiClipPathPlanError::UnsupportedPathCommand { command: other })
            }
            other => Err(UiClipPathPlanError::MalformedPath {
                reason: format!("invalid path command `{other}`").into_boxed_str(),
            }),
        }
    }

    fn move_to(&mut self, command: char, path: &mut ParsedPath) -> Result<(), UiClipPathPlanError> {
        let relative = command.is_ascii_lowercase();
        let first = self.required_point(relative, command)?;
        self.current = first;
        self.subpath_start = Some(first);
        path.push_command(UiClipPathCommandPlan::MoveTo(first))?;

        while self.peek_is_number() {
            let to = self.required_point(relative, command)?;
            let from = self.current;
            path.push_command(UiClipPathCommandPlan::LineTo { from, to })?;
            path.push_edge(command, from, to)?;
            self.current = to;
        }
        Ok(())
    }

    fn line_to(&mut self, command: char, path: &mut ParsedPath) -> Result<(), UiClipPathPlanError> {
        let relative = command.is_ascii_lowercase();
        self.require_number(command)?;
        while self.peek_is_number() {
            let to = self.required_point(relative, command)?;
            let from = self.current;
            path.push_command(UiClipPathCommandPlan::LineTo { from, to })?;
            path.push_edge(command, from, to)?;
            self.current = to;
        }
        Ok(())
    }

    fn horizontal_line_to(
        &mut self,
        command: char,
        path: &mut ParsedPath,
    ) -> Result<(), UiClipPathPlanError> {
        let relative = command.is_ascii_lowercase();
        self.require_number(command)?;
        while self.peek_is_number() {
            let value = self.number(command)?;
            let x = if relative {
                self.current.x + value
            } else {
                self.bounds.x + value
            };
            let to = UiClipVertex {
                x,
                y: self.current.y,
            };
            let from = self.current;
            path.push_command(UiClipPathCommandPlan::LineTo { from, to })?;
            path.push_edge(command, from, to)?;
            self.current = to;
        }
        Ok(())
    }

    fn vertical_line_to(
        &mut self,
        command: char,
        path: &mut ParsedPath,
    ) -> Result<(), UiClipPathPlanError> {
        let relative = command.is_ascii_lowercase();
        self.require_number(command)?;
        while self.peek_is_number() {
            let value = self.number(command)?;
            let y = if relative {
                self.current.y + value
            } else {
                self.bounds.y + value
            };
            let to = UiClipVertex {
                x: self.current.x,
                y,
            };
            let from = self.current;
            path.push_command(UiClipPathCommandPlan::LineTo { from, to })?;
            path.push_edge(command, from, to)?;
            self.current = to;
        }
        Ok(())
    }

    fn quadratic_to(
        &mut self,
        command: char,
        path: &mut ParsedPath,
    ) -> Result<(), UiClipPathPlanError> {
        let relative = command.is_ascii_lowercase();
        self.require_number(command)?;
        while self.peek_is_number() {
            let from = self.current;
            let control = self.required_point(relative, command)?;
            let to = self.required_point(relative, command)?;
            path.push_command(UiClipPathCommandPlan::QuadraticTo {
                from,
                control,
                to,
                subdivisions: PATH_CURVE_SUBDIVISIONS,
            })?;
            flatten_quadratic(command, from, control, to, path)?;
            self.current = to;
        }
        Ok(())
    }

    fn cubic_to(
        &mut self,
        command: char,
        path: &mut ParsedPath,
    ) -> Result<(), UiClipPathPlanError> {
        let relative = command.is_ascii_lowercase();
        self.require_number(command)?;
        while self.peek_is_number() {
            let from = self.current;
            let control_0 = self.required_point(relative, command)?;
            let control_1 = self.required_point(relative, command)?;
            let to = self.required_point(relative, command)?;
            path.push_command(UiClipPathCommandPlan::CubicTo {
                from,
                control_0,
                control_1,
                to,
                subdivisions: PATH_CURVE_SUBDIVISIONS,
            })?;
            flatten_cubic(command, from, control_0, control_1, to, path)?;
            self.current = to;
        }
        Ok(())
    }

    fn close_path(
        &mut self,
        command: char,
        path: &mut ParsedPath,
    ) -> Result<(), UiClipPathPlanError> {
        let Some(to) = self.subpath_start else {
            return Err(UiClipPathPlanError::MalformedPath {
                reason: "close-path command appeared before move-to".into(),
            });
        };
        let from = self.current;
        path.push_command(UiClipPathCommandPlan::ClosePath { from, to })?;
        path.push_edge(command, from, to)?;
        self.current = to;
        Ok(())
    }

    fn required_point(
        &mut self,
        relative: bool,
        command: char,
    ) -> Result<UiClipVertex, UiClipPathPlanError> {
        let x = self.number(command)?;
        let y = self.number(command)?;
        Ok(if relative {
            UiClipVertex {
                x: self.current.x + x,
                y: self.current.y + y,
            }
        } else {
            UiClipVertex {
                x: self.bounds.x + x,
                y: self.bounds.y + y,
            }
        })
    }

    fn number(&mut self, command: char) -> Result<f32, UiClipPathPlanError> {
        match self.peek().copied() {
            Some(PathToken::Number(value)) => {
                self.index += 1;
                if value.is_finite() {
                    Ok(value)
                } else {
                    Err(UiClipPathPlanError::MalformedPath {
                        reason: format!("command `{command}` contains non-finite number")
                            .into_boxed_str(),
                    })
                }
            }
            _ => Err(UiClipPathPlanError::MalformedPath {
                reason: format!("command `{command}` is missing a numeric parameter")
                    .into_boxed_str(),
            }),
        }
    }

    fn require_number(&self, command: char) -> Result<(), UiClipPathPlanError> {
        if self.peek_is_number() {
            Ok(())
        } else {
            Err(UiClipPathPlanError::MalformedPath {
                reason: format!("command `{command}` has no parameters").into_boxed_str(),
            })
        }
    }

    fn peek(&self) -> Option<&PathToken> {
        self.tokens.get(self.index)
    }

    fn peek_is_number(&self) -> bool {
        matches!(self.peek(), Some(PathToken::Number(_)))
    }
}

fn implicit_repeat_command(command: char) -> char {
    match command {
        'M' => 'L',
        'm' => 'l',
        other => other,
    }
}

fn tokenize_path(data: &str) -> Result<Vec<PathToken>, UiClipPathPlanError> {
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < data.len() {
        let Some(ch) = data[index..].chars().next() else {
            break;
        };
        if ch.is_ascii_whitespace() || ch == ',' {
            index += ch.len_utf8();
            continue;
        }
        if ch.is_ascii_alphabetic() {
            tokens.push(PathToken::Command(ch));
            index += ch.len_utf8();
            continue;
        }
        if is_number_start(ch) {
            let (number, next) = parse_number(data, index)?;
            tokens.push(PathToken::Number(number));
            index = next;
            continue;
        }
        return Err(UiClipPathPlanError::MalformedPath {
            reason: format!("unexpected character `{ch}` in path data").into_boxed_str(),
        });
    }
    Ok(tokens)
}

fn parse_number(data: &str, start: usize) -> Result<(f32, usize), UiClipPathPlanError> {
    let mut end = start;
    let mut seen_digit = false;
    let mut seen_dot = false;
    let mut seen_exponent = false;
    let mut previous_was_exponent = false;

    while end < data.len() {
        let Some(ch) = data[end..].chars().next() else {
            break;
        };
        match ch {
            '0'..='9' => {
                seen_digit = true;
                previous_was_exponent = false;
                end += ch.len_utf8();
            }
            '.' if !seen_dot && !seen_exponent => {
                seen_dot = true;
                previous_was_exponent = false;
                end += ch.len_utf8();
            }
            'e' | 'E' if !seen_exponent && seen_digit => {
                seen_exponent = true;
                previous_was_exponent = true;
                end += ch.len_utf8();
            }
            '+' | '-' if end == start || previous_was_exponent => {
                previous_was_exponent = false;
                end += ch.len_utf8();
            }
            _ => break,
        }
    }

    let raw = &data[start..end];
    if raw.is_empty() || !seen_digit {
        return Err(UiClipPathPlanError::MalformedPath {
            reason: "invalid path number".into(),
        });
    }
    raw.parse::<f32>()
        .map(|value| (value, end))
        .map_err(|_| UiClipPathPlanError::MalformedPath {
            reason: format!("invalid path number `{raw}`").into_boxed_str(),
        })
}

fn is_number_start(ch: char) -> bool {
    ch.is_ascii_digit() || ch == '+' || ch == '-' || ch == '.'
}

fn flatten_quadratic(
    command: char,
    from: UiClipVertex,
    control: UiClipVertex,
    to: UiClipVertex,
    path: &mut ParsedPath,
) -> Result<(), UiClipPathPlanError> {
    let mut previous = from;
    for step in 1..=PATH_CURVE_SUBDIVISIONS {
        let progress = f32::from(step) / f32::from(PATH_CURVE_SUBDIVISIONS);
        let current = quadratic_point(from, control, to, progress);
        path.push_edge(command, previous, current)?;
        previous = current;
    }
    Ok(())
}

fn flatten_cubic(
    command: char,
    from: UiClipVertex,
    control_0: UiClipVertex,
    control_1: UiClipVertex,
    to: UiClipVertex,
    path: &mut ParsedPath,
) -> Result<(), UiClipPathPlanError> {
    let mut previous = from;
    for step in 1..=PATH_CURVE_SUBDIVISIONS {
        let progress = f32::from(step) / f32::from(PATH_CURVE_SUBDIVISIONS);
        let current = cubic_point(from, control_0, control_1, to, progress);
        path.push_edge(command, previous, current)?;
        previous = current;
    }
    Ok(())
}

fn quadratic_point(
    from: UiClipVertex,
    control: UiClipVertex,
    to: UiClipVertex,
    progress: f32,
) -> UiClipVertex {
    let one_minus = 1.0 - progress;
    UiClipVertex {
        x: one_minus * one_minus * from.x
            + 2.0 * one_minus * progress * control.x
            + progress * progress * to.x,
        y: one_minus * one_minus * from.y
            + 2.0 * one_minus * progress * control.y
            + progress * progress * to.y,
    }
}

fn cubic_point(
    from: UiClipVertex,
    control_0: UiClipVertex,
    control_1: UiClipVertex,
    to: UiClipVertex,
    progress: f32,
) -> UiClipVertex {
    let one_minus = 1.0 - progress;
    let from_weight = one_minus * one_minus * one_minus;
    let control_0_weight = 3.0 * one_minus * one_minus * progress;
    let control_1_weight = 3.0 * one_minus * progress * progress;
    let to_weight = progress * progress * progress;
    UiClipVertex {
        x: from_weight * from.x
            + control_0_weight * control_0.x
            + control_1_weight * control_1.x
            + to_weight * to.x,
        y: from_weight * from.y
            + control_0_weight * control_0.y
            + control_1_weight * control_1.y
            + to_weight * to.y,
    }
}

fn distance_squared(from: UiClipVertex, to: UiClipVertex) -> f32 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_scene::{UiFillRule, UiLength, UiPoint};

    #[test]
    fn polygon_clip_points_resolve_against_bounds() {
        let clip = UiClipPath::Polygon {
            fill_rule: UiFillRule::EvenOdd,
            points: vec![
                UiPoint::percent(0.0, 0.0),
                UiPoint::percent(1.0, 0.0),
                UiPoint::percent(0.5, 1.0),
            ],
        };

        let plan =
            UiClipGeometryPlan::from_clip_path(Some(&clip), HitRect::new(10.0, 20.0, 100.0, 50.0))
                .expect("polygon resolves");

        assert_eq!(
            plan,
            UiClipGeometryPlan::Polygon {
                fill_rule: UiFillRule::EvenOdd,
                vertices: vec![
                    UiClipVertex { x: 10.0, y: 20.0 },
                    UiClipVertex { x: 110.0, y: 20.0 },
                    UiClipVertex { x: 60.0, y: 70.0 },
                ],
            }
        );
    }

    #[test]
    fn path_clip_plans_lines_curves_and_close_path() {
        let clip = UiClipPath::Path {
            fill_rule: UiFillRule::NonZero,
            data: "M0 0 L20 0 Q30 10 20 20 C10 30 0 30 0 20 Z".into(),
        };

        let plan =
            UiClipGeometryPlan::from_clip_path(Some(&clip), HitRect::new(4.0, 8.0, 40.0, 40.0))
                .expect("path resolves");
        let UiClipGeometryPlan::Path {
            fill_rule,
            commands,
            edges,
        } = plan
        else {
            panic!("expected path plan");
        };

        assert_eq!(fill_rule, UiFillRule::NonZero);
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, UiClipPathCommandPlan::QuadraticTo { .. }))
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, UiClipPathCommandPlan::CubicTo { .. }))
        );
        assert!(edges.len() > 4);
        assert_eq!(edges[0].from, UiClipVertex { x: 4.0, y: 8.0 });
    }

    #[test]
    fn degenerate_path_segments_are_typed_diagnostics() {
        let clip = UiClipPath::Path {
            fill_rule: UiFillRule::NonZero,
            data: "M0 0 L0 0".into(),
        };

        assert_eq!(
            UiClipGeometryPlan::from_clip_path(Some(&clip), HitRect::new(0.0, 0.0, 10.0, 10.0)),
            Err(UiClipPathPlanError::DegeneratePathSegment {
                command: 'L',
                index: 0,
            })
        );
    }

    #[test]
    fn inset_clip_resolves_percent_and_px() {
        let clip = UiClipPath::Inset {
            inset: [
                UiLength::Px(2.0),
                UiLength::Percent(0.1),
                UiLength::Px(4.0),
                UiLength::Percent(0.2),
            ],
            radius: [
                UiLength::Px(1.0),
                UiLength::Px(2.0),
                UiLength::Px(3.0),
                UiLength::Px(4.0),
            ],
        };

        let plan =
            UiClipGeometryPlan::from_clip_path(Some(&clip), HitRect::new(0.0, 0.0, 100.0, 40.0))
                .expect("inset resolves");

        assert_eq!(
            plan,
            UiClipGeometryPlan::Inset {
                rect: HitRect::new(20.0, 2.0, 70.0, 34.0),
                radii_px: [1.0, 2.0, 3.0, 4.0],
            }
        );
    }
}
