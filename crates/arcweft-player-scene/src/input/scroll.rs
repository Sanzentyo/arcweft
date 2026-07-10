use super::{
    InputController, InputOutcome, KEYBOARD_PAGE_SCROLL_FRACTION, PreparedFrame, RenderScrollAxis,
    RenderScrollOverscrollPolicy, RenderScrollRegion, SCROLL_DELTA_EPSILON, ScrollOffset,
    ScrollState, ViewportPoint, finite_delta,
};
use std::time::Duration;

const ELASTIC_RESISTANCE: f32 = 0.35;
const ELASTIC_MAX_DISPLACEMENT_PX: f32 = 96.0;
const ELASTIC_SPRING_FREQUENCY: f32 = 18.0;
const ELASTIC_SETTLE_DISPLACEMENT_PX: f32 = 0.01;
const ELASTIC_SETTLE_VELOCITY_PX_PER_SECOND: f32 = 0.05;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScrollDeltaComponent {
    X,
    Y,
}

impl InputController {
    pub fn wheel(&mut self, frame: &PreparedFrame, delta_y: f32) -> InputOutcome {
        self.precision_scroll(frame, 0.0, delta_y)
    }

    pub fn precision_scroll(
        &mut self,
        frame: &PreparedFrame,
        delta_x: f32,
        delta_y: f32,
    ) -> InputOutcome {
        let regions = self.scroll_chain_for_pointer_or_focus(frame);
        InputOutcome::redraw(self.scroll_chain(frame, &regions, delta_x, delta_y))
    }

    pub fn scroll_region_by_id(
        &mut self,
        frame: &PreparedFrame,
        region_id: &str,
        delta_x: f32,
        delta_y: f32,
    ) -> InputOutcome {
        let Some(region) = frame
            .scroll_regions
            .iter()
            .find(|region| region.id == region_id)
            .cloned()
        else {
            return InputOutcome::redraw(false);
        };
        InputOutcome::redraw(self.scroll_chain(
            frame,
            std::slice::from_ref(&region),
            delta_x,
            delta_y,
        ))
    }

    pub(super) fn scroll_focus_or_pointer_page(
        &mut self,
        frame: &PreparedFrame,
        sign: f32,
    ) -> InputOutcome {
        let Some(region) = self
            .scroll_chain_for_pointer_or_focus(frame)
            .first()
            .cloned()
        else {
            return InputOutcome::default();
        };
        let (delta_x, delta_y) = match region.axis {
            RenderScrollAxis::Vertical => (
                0.0,
                -sign * region.bounds.height * KEYBOARD_PAGE_SCROLL_FRACTION,
            ),
            RenderScrollAxis::Horizontal => (
                -sign * region.bounds.width * KEYBOARD_PAGE_SCROLL_FRACTION,
                0.0,
            ),
        };
        InputOutcome::redraw(self.scroll_chain(
            frame,
            std::slice::from_ref(&region),
            delta_x,
            delta_y,
        ))
    }

    pub(super) fn scroll_focus_or_pointer_to_edge(
        &mut self,
        frame: &PreparedFrame,
        end: bool,
    ) -> InputOutcome {
        let Some(region) = self
            .scroll_chain_for_pointer_or_focus(frame)
            .first()
            .cloned()
        else {
            return InputOutcome::default();
        };
        let next = if end {
            ScrollOffset::new(region.max_offset_x(), region.max_offset_y())
        } else {
            ScrollOffset::new(0.0, 0.0)
        };
        InputOutcome::redraw(self.store_scroll_offset(&region.id, next, frame.visual_time_millis))
    }

    pub(crate) fn resolve_scroll_region(
        &mut self,
        region: &mut RenderScrollRegion,
        visual_time_millis: u64,
        reduce_motion: bool,
    ) {
        let Some(state) = self.scroll_states.get_mut(&region.id) else {
            return;
        };
        state.offset = ScrollOffset::new(
            region.clamped_offset_x(state.offset.x),
            region.clamped_offset_y(state.offset.y),
        );
        match region.axis {
            RenderScrollAxis::Vertical => {
                state.offset.x = 0.0;
                state.overscroll.x = 0.0;
                state.velocity.x = 0.0;
            }
            RenderScrollAxis::Horizontal => {
                state.offset.y = 0.0;
                state.overscroll.y = 0.0;
                state.velocity.y = 0.0;
            }
        }
        state.advance_spring(visual_time_millis, reduce_motion);
        region.offset_x = state.offset.x;
        region.offset_y = state.offset.y;
        region.overscroll_x = state.overscroll.x;
        region.overscroll_y = state.overscroll.y;
        region.indicator_activity_millis = state.activity_millis;
    }

    pub(super) fn mark_scroll_activity(&mut self, region_id: &str, visual_time_millis: u64) {
        self.scroll_states
            .entry(region_id.to_owned())
            .or_default()
            .activity_millis = Some(visual_time_millis);
    }

    fn scroll_chain_for_pointer_or_focus(&self, frame: &PreparedFrame) -> Vec<RenderScrollRegion> {
        self.primary_pointer_position()
            .map(|position| scroll_chain_at_point(frame, position))
            .filter(|chain| !chain.is_empty())
            .or_else(|| {
                self.interaction
                    .focus()
                    .target()
                    .and_then(|target| frame.target_bounds(target))
                    .map(|bounds| {
                        scroll_chain_at_point(
                            frame,
                            ViewportPoint::new(
                                bounds.x + bounds.width * 0.5,
                                bounds.y + bounds.height * 0.5,
                            ),
                        )
                    })
            })
            .unwrap_or_default()
    }

    fn scroll_chain(
        &mut self,
        frame: &PreparedFrame,
        regions: &[RenderScrollRegion],
        delta_x: f32,
        delta_y: f32,
    ) -> bool {
        let mut remaining_x = finite_delta(delta_x);
        let mut remaining_y = finite_delta(delta_y);
        let mut changed = false;
        for region in regions {
            let (component, delta) = match region.axis {
                RenderScrollAxis::Horizontal if remaining_x.abs() > SCROLL_DELTA_EPSILON => {
                    (ScrollDeltaComponent::X, remaining_x)
                }
                RenderScrollAxis::Vertical | RenderScrollAxis::Horizontal => {
                    (ScrollDeltaComponent::Y, remaining_y)
                }
            };
            let (region_changed, remainder) = self.scroll_region(
                region,
                delta,
                frame.visual_time_millis,
                frame.preferences.reduce_motion,
            );
            changed |= region_changed;
            match component {
                ScrollDeltaComponent::X => remaining_x = remainder,
                ScrollDeltaComponent::Y => remaining_y = remainder,
            }
        }
        changed
    }

    /// Applies one input-space delta and returns the part that may chain to an ancestor.
    fn scroll_region(
        &mut self,
        region: &RenderScrollRegion,
        input_delta: f32,
        visual_time_millis: u64,
        reduce_motion: bool,
    ) -> (bool, f32) {
        if !region.overflow.scroll_enabled() || input_delta.abs() <= SCROLL_DELTA_EPSILON {
            return (false, input_delta);
        }
        let max_offset = match region.axis {
            RenderScrollAxis::Vertical => region.max_offset_y(),
            RenderScrollAxis::Horizontal => region.max_offset_x(),
        };
        if max_offset <= f32::EPSILON {
            return (false, input_delta);
        }

        let state = self
            .scroll_states
            .entry(region.id.clone())
            .or_insert_with(|| ScrollState {
                offset: ScrollOffset::new(region.offset_x, region.offset_y),
                ..ScrollState::default()
            });
        state.advance_spring(visual_time_millis, reduce_motion);
        let desired_delta = -input_delta;
        let current = match region.axis {
            RenderScrollAxis::Vertical => state.offset.y,
            RenderScrollAxis::Horizontal => state.offset.x,
        };
        let next = (current + desired_delta).clamp(0.0, max_offset);
        let consumed = next - current;
        let excess = desired_delta - consumed;
        let mut changed = consumed.abs() > SCROLL_DELTA_EPSILON;

        if changed {
            match region.axis {
                RenderScrollAxis::Vertical => {
                    state.offset.y = next;
                    state.overscroll.y = 0.0;
                    state.velocity.y = 0.0;
                }
                RenderScrollAxis::Horizontal => {
                    state.offset.x = next;
                    state.overscroll.x = 0.0;
                    state.velocity.x = 0.0;
                }
            }
        }

        let remainder = if excess.abs() <= SCROLL_DELTA_EPSILON {
            0.0
        } else {
            match region.overscroll {
                RenderScrollOverscrollPolicy::Clamp => -excess,
                RenderScrollOverscrollPolicy::Contain => 0.0,
                RenderScrollOverscrollPolicy::Elastic => {
                    if !reduce_motion {
                        changed |= state.add_elastic_displacement(
                            region.axis,
                            excess,
                            elastic_limit(region),
                            visual_time_millis,
                        );
                    }
                    0.0
                }
            }
        };
        let activity_changed = state.activity_millis != Some(visual_time_millis);
        state.activity_millis = Some(visual_time_millis);
        changed |= activity_changed;
        (changed, remainder)
    }

    pub(super) fn store_scroll_offset(
        &mut self,
        region_id: &str,
        next: ScrollOffset,
        visual_time_millis: u64,
    ) -> bool {
        let state = self.scroll_states.entry(region_id.to_owned()).or_default();
        let before = state.offset;
        let activity_changed = state.activity_millis != Some(visual_time_millis);
        state.offset = next;
        state.overscroll = ScrollOffset::default();
        state.velocity = ScrollOffset::default();
        state.spring_time_millis = Some(visual_time_millis);
        state.activity_millis = Some(visual_time_millis);
        before != next || activity_changed
    }

    fn primary_pointer_position(&self) -> Option<ViewportPoint> {
        self.pointer_positions.values().next().copied()
    }
}

impl ScrollState {
    fn advance_spring(&mut self, visual_time_millis: u64, reduce_motion: bool) {
        let Some(previous) = self.spring_time_millis.replace(visual_time_millis) else {
            if reduce_motion {
                self.clear_spring();
            }
            return;
        };
        if reduce_motion {
            self.clear_spring();
            return;
        }
        let elapsed = visual_time_millis.saturating_sub(previous);
        if elapsed == 0 {
            return;
        }
        let seconds = Duration::from_millis(elapsed).as_secs_f32();
        (self.overscroll.x, self.velocity.x) =
            advance_critical_spring(self.overscroll.x, self.velocity.x, seconds);
        (self.overscroll.y, self.velocity.y) =
            advance_critical_spring(self.overscroll.y, self.velocity.y, seconds);
    }

    fn add_elastic_displacement(
        &mut self,
        axis: RenderScrollAxis,
        excess: f32,
        limit: f32,
        visual_time_millis: u64,
    ) -> bool {
        let displacement = match axis {
            RenderScrollAxis::Vertical => &mut self.overscroll.y,
            RenderScrollAxis::Horizontal => &mut self.overscroll.x,
        };
        let before = *displacement;
        *displacement = (*displacement + excess * ELASTIC_RESISTANCE).clamp(-limit, limit);
        match axis {
            RenderScrollAxis::Vertical => self.velocity.y = 0.0,
            RenderScrollAxis::Horizontal => self.velocity.x = 0.0,
        }
        self.spring_time_millis = Some(visual_time_millis);
        (*displacement - before).abs() > SCROLL_DELTA_EPSILON
    }

    fn clear_spring(&mut self) {
        self.overscroll = ScrollOffset::default();
        self.velocity = ScrollOffset::default();
    }
}

fn scroll_chain_at_point(frame: &PreparedFrame, point: ViewportPoint) -> Vec<RenderScrollRegion> {
    let mut candidates = frame
        .scroll_regions
        .iter()
        .enumerate()
        .filter(|(_, region)| region.overflow.scroll_enabled() && region.contains(point))
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_index, left), (right_index, right)| {
        scroll_region_area(left)
            .total_cmp(&scroll_region_area(right))
            .then_with(|| right_index.cmp(left_index))
    });
    let Some((_, innermost)) = candidates.first() else {
        return Vec::new();
    };
    let mut chain = vec![(*innermost).clone()];
    for (_, candidate) in candidates.into_iter().skip(1) {
        if chain
            .last()
            .is_some_and(|inner| inner.is_contained_by(candidate))
        {
            chain.push(candidate.clone());
        }
    }
    chain
}

fn scroll_region_area(region: &RenderScrollRegion) -> f32 {
    region.bounds.width.max(0.0) * region.bounds.height.max(0.0)
}

fn elastic_limit(region: &RenderScrollRegion) -> f32 {
    let viewport = match region.axis {
        RenderScrollAxis::Vertical => region.bounds.height,
        RenderScrollAxis::Horizontal => region.bounds.width,
    };
    (viewport.max(0.0) * 0.25).min(ELASTIC_MAX_DISPLACEMENT_PX)
}

fn advance_critical_spring(displacement: f32, velocity: f32, seconds: f32) -> (f32, f32) {
    if displacement.abs() <= ELASTIC_SETTLE_DISPLACEMENT_PX
        && velocity.abs() <= ELASTIC_SETTLE_VELOCITY_PX_PER_SECOND
    {
        return (0.0, 0.0);
    }
    let coefficient = velocity + ELASTIC_SPRING_FREQUENCY * displacement;
    let decay = (-ELASTIC_SPRING_FREQUENCY * seconds).exp();
    let next_displacement = (displacement + coefficient * seconds) * decay;
    let next_velocity = (velocity - ELASTIC_SPRING_FREQUENCY * coefficient * seconds) * decay;
    if next_displacement.abs() <= ELASTIC_SETTLE_DISPLACEMENT_PX
        && next_velocity.abs() <= ELASTIC_SETTLE_VELOCITY_PX_PER_SECOND
    {
        (0.0, 0.0)
    } else {
        (next_displacement, next_velocity)
    }
}

#[cfg(test)]
mod spring_tests {
    use super::advance_critical_spring;

    #[test]
    fn critical_spring_is_partition_invariant() {
        let whole = advance_critical_spring(32.0, 0.0, 0.25);
        let first = advance_critical_spring(32.0, 0.0, 0.1);
        let partitioned = advance_critical_spring(first.0, first.1, 0.15);
        assert!((whole.0 - partitioned.0).abs() < 0.0001);
        assert!((whole.1 - partitioned.1).abs() < 0.0001);
    }
}
