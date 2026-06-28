//! Clip-path geometry planning for the UI compositor.

use crate::ui_scene::{UiClipPath, UiFillRule, UiLength, UiPoint, UiShapeRadius};
use arcweft_presentation::hit::HitRect;
use thiserror::Error;

/// Device-independent clip geometry consumed by stencil or analytic shader paths.
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiClipVertex {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UiClipPathPlanError {
    #[error(
        "CSS path() clip-path requires a vector path tessellator and is not enabled in seq06.9b"
    )]
    PathUnsupported,
    #[error("clip-path value `{0}` has no GPU geometry lowering in seq06.9b")]
    Unsupported(Box<str>),
    #[error("clip-path length `{0}` cannot be resolved against the current bounds")]
    UnresolvableLength(Box<str>),
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
            UiClipPath::Polygon { fill_rule, points } => Ok(Self::Polygon {
                fill_rule: *fill_rule,
                vertices: points
                    .iter()
                    .map(|point| resolve_point(point, bounds))
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            UiClipPath::Path { .. } => Err(UiClipPathPlanError::PathUnsupported),
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
    fn path_clip_stays_explicit_until_tessellator_lands() {
        let clip = UiClipPath::Path {
            fill_rule: UiFillRule::NonZero,
            data: "M0 0 L1 1".into(),
        };

        assert_eq!(
            UiClipGeometryPlan::from_clip_path(Some(&clip), HitRect::new(0.0, 0.0, 10.0, 10.0)),
            Err(UiClipPathPlanError::PathUnsupported)
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
