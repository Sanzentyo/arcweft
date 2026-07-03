//! Box-shadow pass planning for Arcweft UI compositing.
//!
//! CSS `box-shadow` is distinct from `filter: drop-shadow(...)`: box-shadow is
//! generated from a box/radius/spread list, while drop-shadow is generated from
//! rendered subtree alpha. This module is pure renderer planning data and has no
//! GPU, filesystem, DOM, canvas, or Takumi raster dependency.

use crate::ui_scene::{UiBoxShadow, UiBoxShadowKind, UiBoxShadowList};
use arcweft_presentation::hit::HitRect;
use thiserror::Error;

const EPSILON: f32 = 0.0001;

/// Ordered box-shadow pass list for one compositing group.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiBoxShadowPassPlan {
    passes: Vec<UiBoxShadowPass>,
    visual_outset_px: f32,
    visual_inset_px: f32,
}

/// One box-shadow draw pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiBoxShadowPass {
    /// Original CSS-list index. Lower indices are visually above higher indices.
    pub shadow_index: usize,
    pub shadow: UiBoxShadow,
    pub body_rect: HitRect,
    pub shadow_rect: HitRect,
    pub body_radius_px: f32,
    pub shadow_radius_px: f32,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum UiBoxShadowPlanError {
    #[error("box-shadow at index {shadow_index} has degenerate {kind:?} geometry: {reason}")]
    DegenerateGeometry {
        shadow_index: usize,
        kind: UiBoxShadowKind,
        reason: &'static str,
    },
    #[error("box-shadow at index {shadow_index} has non-finite `{field}`")]
    NonFinite {
        shadow_index: usize,
        field: &'static str,
    },
}

impl UiBoxShadowPassPlan {
    pub fn from_shadows(
        shadows: &UiBoxShadowList,
        bounds: HitRect,
    ) -> Result<Self, UiBoxShadowPlanError> {
        let mut passes = Vec::new();
        for (shadow_index, shadow) in shadows.shadows().iter().copied().enumerate().rev() {
            validate_shadow(shadow_index, shadow)?;
            if shadow.is_identity() {
                continue;
            }
            match shadow.kind {
                UiBoxShadowKind::Outer => {
                    if let Some(pass) =
                        UiBoxShadowPass::from_outer_shadow(shadow_index, shadow, bounds)
                    {
                        passes.push(pass);
                    }
                }
                UiBoxShadowKind::Inset => {
                    passes.push(UiBoxShadowPass::from_inset_shadow(
                        shadow_index,
                        shadow,
                        bounds,
                    )?);
                }
            }
        }
        Ok(Self {
            passes,
            visual_outset_px: shadows.visual_outset_px(),
            visual_inset_px: shadows.visual_inset_px(),
        })
    }

    pub fn passes(&self) -> &[UiBoxShadowPass] {
        &self.passes
    }

    pub fn passes_for_kind(
        &self,
        kind: UiBoxShadowKind,
    ) -> impl Iterator<Item = &UiBoxShadowPass> + '_ {
        self.passes
            .iter()
            .filter(move |pass| pass.shadow.kind == kind)
    }

    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    pub const fn visual_outset_px(&self) -> f32 {
        self.visual_outset_px
    }

    pub const fn visual_inset_px(&self) -> f32 {
        self.visual_inset_px
    }
}

impl UiBoxShadowPass {
    fn from_outer_shadow(
        shadow_index: usize,
        shadow: UiBoxShadow,
        bounds: HitRect,
    ) -> Option<Self> {
        let mut shadow_rect = outset_rect(bounds, shadow.spread_radius_px);
        if shadow_rect.width <= EPSILON || shadow_rect.height <= EPSILON {
            return None;
        }
        shadow_rect.x += shadow.offset_x_px;
        shadow_rect.y += shadow.offset_y_px;

        let body_radius_px = clamp_radius(shadow.border_radius_px, bounds);
        let shadow_radius_px = clamp_radius(
            (shadow.border_radius_px + shadow.spread_radius_px).max(0.0),
            shadow_rect,
        );

        Some(Self {
            shadow_index,
            shadow,
            body_rect: bounds,
            shadow_rect,
            body_radius_px,
            shadow_radius_px,
        })
    }

    fn from_inset_shadow(
        shadow_index: usize,
        shadow: UiBoxShadow,
        bounds: HitRect,
    ) -> Result<Self, UiBoxShadowPlanError> {
        if bounds.width <= EPSILON || bounds.height <= EPSILON {
            return Err(UiBoxShadowPlanError::DegenerateGeometry {
                shadow_index,
                kind: UiBoxShadowKind::Inset,
                reason: "inset receiver bounds have no drawable area",
            });
        }

        let mut shadow_rect = outset_rect(bounds, -shadow.spread_radius_px);
        shadow_rect.x += shadow.offset_x_px;
        shadow_rect.y += shadow.offset_y_px;

        let body_radius_px = clamp_radius(shadow.border_radius_px, bounds);
        let shadow_radius_px = clamp_radius(
            (shadow.border_radius_px - shadow.spread_radius_px).max(0.0),
            shadow_rect,
        );

        Ok(Self {
            shadow_index,
            shadow,
            body_rect: bounds,
            shadow_rect,
            body_radius_px,
            shadow_radius_px,
        })
    }
}

fn validate_shadow(shadow_index: usize, shadow: UiBoxShadow) -> Result<(), UiBoxShadowPlanError> {
    for (field, value) in [
        ("offset_x_px", shadow.offset_x_px),
        ("offset_y_px", shadow.offset_y_px),
        ("blur_radius_px", shadow.blur_radius_px),
        ("spread_radius_px", shadow.spread_radius_px),
        ("border_radius_px", shadow.border_radius_px),
    ] {
        if !value.is_finite() {
            return Err(UiBoxShadowPlanError::NonFinite {
                shadow_index,
                field,
            });
        }
    }
    Ok(())
}

fn outset_rect(bounds: HitRect, outset_px: f32) -> HitRect {
    HitRect::new(
        bounds.x - outset_px,
        bounds.y - outset_px,
        (bounds.width + outset_px * 2.0).max(0.0),
        (bounds.height + outset_px * 2.0).max(0.0),
    )
}

fn clamp_radius(radius_px: f32, bounds: HitRect) -> f32 {
    radius_px
        .max(0.0)
        .min(bounds.width.max(0.0).min(bounds.height.max(0.0)) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_scene::{UiBoxShadow, UiColorRgba8};

    fn rgba(alpha: u8) -> UiColorRgba8 {
        UiColorRgba8 {
            red: 20,
            green: 30,
            blue: 40,
            alpha,
        }
    }

    #[test]
    fn transparent_and_zero_outer_shadows_are_canonical_noops() {
        let shadows = UiBoxShadowList::new([
            UiBoxShadow::outer(0.0, 0.0, 0.0, 0.0, 8.0, rgba(255)),
            UiBoxShadow::outer(12.0, 0.0, 2.0, 1.0, 8.0, rgba(0)),
        ]);

        let plan =
            UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(10.0, 20.0, 100.0, 50.0))
                .expect("transparent/zero shadows are valid");

        assert!(plan.is_empty());
    }

    #[test]
    fn transparent_and_zero_inset_shadows_are_canonical_noops() {
        let shadows = UiBoxShadowList::new([
            UiBoxShadow::inset(0.0, 0.0, 0.0, 0.0, 8.0, rgba(255)),
            UiBoxShadow::inset(12.0, 0.0, 2.0, 1.0, 8.0, rgba(0)),
        ]);

        let plan =
            UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(10.0, 20.0, 100.0, 50.0))
                .expect("transparent/zero inset shadows are valid");

        assert!(plan.is_empty());
    }

    #[test]
    fn multiple_outer_shadows_paint_back_to_front() {
        let shadows = UiBoxShadowList::new([
            UiBoxShadow::outer(1.0, 0.0, 2.0, 0.0, 4.0, rgba(120)),
            UiBoxShadow::outer(2.0, 0.0, 2.0, 0.0, 4.0, rgba(130)),
            UiBoxShadow::outer(3.0, 0.0, 2.0, 0.0, 4.0, rgba(140)),
        ]);

        let plan = UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0))
            .expect("outer shadows plan");

        assert_eq!(
            plan.passes()
                .iter()
                .map(|pass| pass.shadow_index)
                .collect::<Vec<_>>(),
            vec![2, 1, 0]
        );
    }

    #[test]
    fn inset_shadow_plans_deterministic_inner_geometry() {
        let shadows =
            UiBoxShadowList::new([UiBoxShadow::inset(2.0, -4.0, 6.0, 3.0, 8.0, rgba(180))]);

        let plan =
            UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(10.0, 20.0, 100.0, 50.0))
                .expect("inset shadow plans");
        let pass = plan.passes()[0];

        assert_eq!(pass.shadow.kind, UiBoxShadowKind::Inset);
        assert_eq!(pass.body_rect, HitRect::new(10.0, 20.0, 100.0, 50.0));
        assert_eq!(pass.shadow_rect, HitRect::new(15.0, 19.0, 94.0, 44.0));
        assert!((pass.body_radius_px - 8.0).abs() <= EPSILON);
        assert!((pass.shadow_radius_px - 5.0).abs() <= EPSILON);
        assert!((plan.visual_outset_px() - 0.0).abs() <= EPSILON);
        assert!((plan.visual_inset_px() - 25.0).abs() <= EPSILON);
    }

    #[test]
    fn multiple_inset_shadows_paint_back_to_front_within_inset_stage() {
        let shadows = UiBoxShadowList::new([
            UiBoxShadow::inset(1.0, 0.0, 2.0, 0.0, 4.0, rgba(120)),
            UiBoxShadow::inset(2.0, 0.0, 2.0, 0.0, 4.0, rgba(130)),
            UiBoxShadow::inset(3.0, 0.0, 2.0, 0.0, 4.0, rgba(140)),
        ]);

        let plan = UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0))
            .expect("inset shadows plan");

        assert_eq!(
            plan.passes_for_kind(UiBoxShadowKind::Inset)
                .map(|pass| pass.shadow_index)
                .collect::<Vec<_>>(),
            vec![2, 1, 0]
        );
    }

    #[test]
    fn spread_changes_shadow_rect_and_outset() {
        let shadows =
            UiBoxShadowList::new([UiBoxShadow::outer(4.0, 6.0, 8.0, 3.0, 10.0, rgba(160))]);

        let plan =
            UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(10.0, 20.0, 100.0, 50.0))
                .expect("spread shadow plans");
        let pass = plan.passes()[0];

        assert_eq!(pass.shadow_rect, HitRect::new(11.0, 23.0, 106.0, 56.0));
        assert!((plan.visual_outset_px() - 33.0).abs() <= EPSILON);
    }

    #[test]
    fn negative_spread_changes_inset_shadow_rect_deterministically() {
        let shadows =
            UiBoxShadowList::new([UiBoxShadow::inset(4.0, -2.0, 8.0, -5.0, 12.0, rgba(160))]);

        let plan = UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0))
            .expect("negative inset spread plans");
        let pass = plan.passes()[0];

        assert_eq!(pass.shadow_rect, HitRect::new(-1.0, -7.0, 90.0, 50.0));
        assert!((pass.shadow_radius_px - 17.0).abs() <= EPSILON);
    }

    #[test]
    fn non_finite_inset_fields_emit_typed_diagnostics() {
        let shadows =
            UiBoxShadowList::new([UiBoxShadow::inset(f32::NAN, 2.0, 6.0, 0.0, 6.0, rgba(180))]);

        assert_eq!(
            UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0)),
            Err(UiBoxShadowPlanError::NonFinite {
                shadow_index: 0,
                field: "offset_x_px",
            })
        );
    }

    #[test]
    fn degenerate_inset_receiver_is_typed_diagnostic() {
        let shadows =
            UiBoxShadowList::new([UiBoxShadow::inset(0.0, 2.0, 6.0, 1.0, 6.0, rgba(180))]);

        assert_eq!(
            UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 0.0, 40.0)),
            Err(UiBoxShadowPlanError::DegenerateGeometry {
                shadow_index: 0,
                kind: UiBoxShadowKind::Inset,
                reason: "inset receiver bounds have no drawable area",
            })
        );
    }

    #[test]
    fn mixed_outer_and_inset_shadows_preserve_stage_order() {
        let shadows = UiBoxShadowList::new([
            UiBoxShadow::outer(1.0, 0.0, 2.0, 0.0, 4.0, rgba(120)),
            UiBoxShadow::inset(2.0, 0.0, 2.0, 0.0, 4.0, rgba(130)),
            UiBoxShadow::outer(3.0, 0.0, 2.0, 0.0, 4.0, rgba(140)),
            UiBoxShadow::inset(4.0, 0.0, 2.0, 0.0, 4.0, rgba(150)),
        ]);

        let plan = UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0))
            .expect("mixed shadows plan");

        assert_eq!(
            plan.passes()
                .iter()
                .map(|pass| pass.shadow_index)
                .collect::<Vec<_>>(),
            vec![3, 2, 1, 0]
        );
        assert_eq!(
            plan.passes_for_kind(UiBoxShadowKind::Outer)
                .map(|pass| pass.shadow_index)
                .collect::<Vec<_>>(),
            vec![2, 0]
        );
        assert_eq!(
            plan.passes_for_kind(UiBoxShadowKind::Inset)
                .map(|pass| pass.shadow_index)
                .collect::<Vec<_>>(),
            vec![3, 1]
        );
    }
}
