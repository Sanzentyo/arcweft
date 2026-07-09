use super::{
    InputController, InputOutcome, KEYBOARD_PAGE_SCROLL_FRACTION, PreparedFrame, RenderScrollAxis,
    RenderScrollRegion, SCROLL_DELTA_EPSILON, ScrollOffset, ViewportPoint, finite_delta,
};

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
        let Some(region) = self.scroll_region_for_pointer_or_focus(frame) else {
            return InputOutcome::redraw(false);
        };
        InputOutcome::redraw(self.scroll_region(region, delta_x, delta_y))
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
        else {
            return InputOutcome::redraw(false);
        };
        InputOutcome::redraw(self.scroll_region(region, delta_x, delta_y))
    }

    pub(super) fn scroll_focus_or_pointer_page(
        &mut self,
        frame: &PreparedFrame,
        sign: f32,
    ) -> InputOutcome {
        let Some(region) = self.scroll_region_for_pointer_or_focus(frame) else {
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
        InputOutcome::redraw(self.scroll_region(region, delta_x, delta_y))
    }

    pub(super) fn scroll_focus_or_pointer_to_edge(
        &mut self,
        frame: &PreparedFrame,
        end: bool,
    ) -> InputOutcome {
        let Some(region) = self.scroll_region_for_pointer_or_focus(frame) else {
            return InputOutcome::default();
        };
        let next = if end {
            ScrollOffset::new(region.max_offset_x(), region.max_offset_y())
        } else {
            ScrollOffset::new(0.0, 0.0)
        };
        InputOutcome::redraw(self.store_scroll_offset(&region.id, next))
    }

    fn scroll_region_for_pointer_or_focus<'a>(
        &self,
        frame: &'a PreparedFrame,
    ) -> Option<&'a RenderScrollRegion> {
        self.primary_pointer_position()
            .and_then(|position| {
                frame
                    .scroll_regions
                    .iter()
                    .rev()
                    .find(|region| region.contains(position))
            })
            .or_else(|| {
                self.interaction
                    .focus()
                    .target()
                    .and_then(|target| frame.scroll_region_for_target(target))
            })
    }

    fn scroll_region(&mut self, region: &RenderScrollRegion, delta_x: f32, delta_y: f32) -> bool {
        let current = self
            .scroll_offsets
            .get(&region.id)
            .copied()
            .unwrap_or_else(|| ScrollOffset::new(region.offset_x, region.offset_y));
        let next = match region.axis {
            RenderScrollAxis::Vertical => ScrollOffset::new(
                0.0,
                region.clamped_offset_y(current.y - finite_delta(delta_y)),
            ),
            RenderScrollAxis::Horizontal => {
                let primary = if delta_x.abs() > SCROLL_DELTA_EPSILON {
                    delta_x
                } else {
                    delta_y
                };
                ScrollOffset::new(
                    region.clamped_offset_x(current.x - finite_delta(primary)),
                    0.0,
                )
            }
        };
        self.store_scroll_offset(&region.id, next)
    }

    pub(super) fn store_scroll_offset(&mut self, region_id: &str, next: ScrollOffset) -> bool {
        let before = self
            .scroll_offsets
            .get(region_id)
            .copied()
            .unwrap_or_default();
        if next.is_zero() {
            self.scroll_offsets.remove(region_id);
        } else {
            self.scroll_offsets.insert(region_id.to_owned(), next);
        }
        before != next
    }

    fn primary_pointer_position(&self) -> Option<ViewportPoint> {
        self.pointer_positions.values().next().copied()
    }
}
