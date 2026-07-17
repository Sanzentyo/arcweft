use super::{
    BTreeMap, ChoiceScroll, FramePlanError, InputController, InputControllerSnapshot,
    InputControllerSnapshotError, InputScrollOffsetSnapshot, InteractionState,
    InteractionVisualState, PointerId, PreparedFrame, RenderTextInputControl, ScrollOffset,
    TextByteOffset, TextEditorState, TextRange, ViewportPoint, text_control_matches_editor,
};

impl InputController {
    pub const fn interaction(&self) -> &InteractionState {
        &self.interaction
    }

    pub fn focus_visible_for(
        &self,
        target: &arcweft_presentation::input::InteractionTarget,
    ) -> bool {
        self.focus_visible && self.interaction.is_focused(target)
    }

    pub const fn choice_scroll(&self) -> ChoiceScroll {
        self.choice_scroll
    }

    pub fn scroll_offset_y(&self, region_id: &str) -> f32 {
        self.scroll_states
            .get(region_id)
            .map_or(0.0, |state| state.offset.y)
    }

    pub fn scroll_offset_x(&self, region_id: &str) -> f32 {
        self.scroll_states
            .get(region_id)
            .map_or(0.0, |state| state.offset.x)
    }

    #[must_use]
    pub fn snapshot(&self) -> InputControllerSnapshot {
        InputControllerSnapshot {
            choice_scroll_offset_y: self.choice_scroll.offset_y,
            scroll_offsets: self
                .scroll_states
                .iter()
                .filter(|(_, state)| !state.offset.is_zero())
                .map(|(region_id, state)| InputScrollOffsetSnapshot {
                    region_id: region_id.clone(),
                    offset_x: state.offset.x,
                    offset_y: state.offset.y,
                })
                .collect(),
        }
    }

    pub fn restore_snapshot(
        &mut self,
        snapshot: InputControllerSnapshot,
    ) -> Result<(), InputControllerSnapshotError> {
        let choice_scroll_offset_y = snapshot.choice_scroll_offset_y;
        if !choice_scroll_offset_y.is_finite() {
            return Err(InputControllerSnapshotError::NonFiniteChoiceScroll {
                offset_y: choice_scroll_offset_y,
            });
        }
        if choice_scroll_offset_y < 0.0 {
            return Err(InputControllerSnapshotError::NegativeChoiceScroll {
                offset_y: choice_scroll_offset_y,
            });
        }
        let scroll_states = snapshot.scroll_offsets.into_iter().try_fold(
            BTreeMap::new(),
            |mut offsets, entry| {
                if entry.region_id.is_empty() {
                    return Err(InputControllerSnapshotError::EmptyScrollRegionId);
                }
                if !entry.offset_x.is_finite() || !entry.offset_y.is_finite() {
                    return Err(InputControllerSnapshotError::NonFiniteScrollOffset {
                        region_id: entry.region_id,
                        offset_x: entry.offset_x,
                        offset_y: entry.offset_y,
                    });
                }
                let offset = ScrollOffset::new(entry.offset_x, entry.offset_y);
                if !offset.is_zero() {
                    offsets.insert(
                        entry.region_id,
                        super::ScrollState {
                            offset,
                            ..super::ScrollState::default()
                        },
                    );
                }
                Ok(offsets)
            },
        )?;
        self.choice_scroll.offset_y = choice_scroll_offset_y;
        self.scroll_states = scroll_states;
        self.controller.reset_transient_state();
        Ok(())
    }

    pub const fn window_focused(&self) -> bool {
        self.window_focused
    }

    pub const fn ime_composing(&self) -> bool {
        self.ime_composing
    }

    pub const fn focused_text_editor(&self) -> Option<&TextEditorState> {
        self.focused_text_editor.as_ref()
    }

    pub(crate) fn text_block_selection_for(
        &self,
        target: &arcweft_presentation::input::InteractionTarget,
        text: &str,
    ) -> Option<TextRange<TextByteOffset>> {
        self.text_block_selection
            .as_ref()
            .filter(|selection| selection.target == *target && selection.text == text)
            .map(|selection| selection.selection)
            .filter(|selection| selection.start() != selection.end())
    }

    pub fn pointer_position(&self, pointer: PointerId) -> Option<ViewportPoint> {
        self.pointer_positions.get(&pointer.0).copied()
    }

    pub fn visual_state(&self) -> InteractionVisualState {
        InteractionVisualState {
            focused: self.interaction.focus().target().cloned(),
            hovered: self.interaction.primary_hovered_target().cloned(),
            pressed: self.interaction.primary_pressed_target().cloned(),
        }
    }

    /// Activates the player-owned editor for a declarative runtime text control.
    ///
    /// Platform bridges still receive focus exclusively from
    /// `PreparedFrame::focused_text_input_target`; this state is the live
    /// product editor that validated platform edits mutate before the next
    /// frame is planned.
    pub fn activate_text_control(
        &mut self,
        control: &RenderTextInputControl,
    ) -> Result<bool, FramePlanError> {
        let options = control.resolved_options()?;
        let already_active = self.focused_text_editor.as_ref().is_some_and(|editor| {
            editor.session() == control.session && editor.target() == &control.target
        });
        if already_active {
            return Ok(false);
        }
        self.focused_text_editor = Some(TextEditorState::from_text_control(
            control.session,
            control.target.clone(),
            control.value.clone(),
            control.selection,
            options,
        )?);
        Ok(true)
    }

    pub(crate) fn retain_live_text_control_focus(&mut self, controls: &[RenderTextInputControl]) {
        let stale = self.focused_text_editor.as_ref().is_some_and(|editor| {
            !controls
                .iter()
                .any(|control| text_control_matches_editor(control, editor))
        });
        if stale {
            self.deactivate_focused_text_editor();
        }
    }

    #[must_use]
    pub fn apply_live_text_control_state(
        &self,
        control: RenderTextInputControl,
    ) -> RenderTextInputControl {
        self.focused_text_editor
            .as_ref()
            .map_or(control.clone(), |editor| {
                if editor.session() == control.session && editor.target() == &control.target {
                    control
                        .with_value(editor.text())
                        .with_selection(editor.selection())
                        .with_options(editor.options().clone())
                } else {
                    control
                }
            })
    }

    pub fn ensure_choice_focus(&mut self, frame: &PreparedFrame) -> bool {
        if self.interaction.focus().target().is_none()
            && self.focused_text_editor.is_none()
            && let Some(target) = frame.choices.first().map(|choice| choice.target.clone())
        {
            self.set_focus(frame, target, true);
            self.interaction.focus().target().is_some()
        } else {
            false
        }
    }
}
