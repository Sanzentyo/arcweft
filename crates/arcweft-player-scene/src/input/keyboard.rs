use super::{
    ActionButtonSubmitOutcome, ControllerInputChange, DialogueProgress, FocusNavigationDirection,
    InputController, InputEpoch, InputEvent, InputOutcome, KeyPhase, NormalizedControllerAction,
    PreparedFrame, RenderActionButtonAction, TextControlWriteBack, TextEditorState,
    TextInputKeyDisposition, activation_outcome, choice_action, dialogue_progress_for_frame,
    frame_target_is_view_control,
};

impl InputController {
    pub fn keyboard(&mut self, frame: &PreparedFrame, key: &str, phase: KeyPhase) -> InputOutcome {
        if phase == KeyPhase::Up {
            return InputOutcome::default();
        }
        match key {
            "ArrowUp" => self.move_focus(frame, FocusNavigationDirection::Up),
            "ArrowDown" => self.move_focus(frame, FocusNavigationDirection::Down),
            "ArrowLeft" => self.move_focus(frame, FocusNavigationDirection::Left),
            "ArrowRight" => self.move_focus(frame, FocusNavigationDirection::Right),
            "Tab" => self.move_focus(frame, FocusNavigationDirection::Next),
            "PageDown" => self.scroll_focus_or_pointer_page(frame, 1.0),
            "PageUp" => self.scroll_focus_or_pointer_page(frame, -1.0),
            "Home" => {
                if self.scroll_focus_or_pointer_to_edge(frame, false).redraw {
                    return InputOutcome::redraw(true);
                }
                if let Some(target) = frame.first_keyboard_focus_target() {
                    self.set_focus(frame, target, true);
                }
                InputOutcome::redraw(true)
            }
            "End" => {
                if self.scroll_focus_or_pointer_to_edge(frame, true).redraw {
                    return InputOutcome::redraw(true);
                }
                if let Some(target) = frame.last_keyboard_focus_target() {
                    self.set_focus(frame, target, true);
                }
                InputOutcome::redraw(true)
            }
            "Enter" | " " | "Space" => self.activate_focused(frame),
            "Backspace" => self.dialogue_advance_from_keyboard(frame),
            _ => InputOutcome::default(),
        }
    }

    pub fn keyboard_with_modifiers(
        &mut self,
        frame: &PreparedFrame,
        key: &str,
        phase: KeyPhase,
        shift: bool,
    ) -> InputOutcome {
        if key == "Tab" && phase == KeyPhase::Down && shift {
            return self.move_focus(frame, FocusNavigationDirection::Previous);
        }
        self.keyboard(frame, key, phase)
    }

    pub fn keyboard_with_ime(
        &mut self,
        frame: &PreparedFrame,
        key: &str,
        phase: KeyPhase,
        disposition: TextInputKeyDisposition,
    ) -> InputOutcome {
        if disposition.shortcuts_suppressed() || self.ime_composing {
            return InputOutcome {
                actions: Vec::new(),
                view_handler_invocations: Vec::new(),
                text_control_write_backs: Vec::new(),
                clipboard_requests: Vec::new(),
                diagnostics: Vec::new(),
                dialogue_progress: DialogueProgress::None,
                cancel: false,
                redraw: self.ime_composing,
            };
        }
        self.keyboard(frame, key, phase)
    }

    pub fn keyboard_with_modifiers_and_ime(
        &mut self,
        frame: &PreparedFrame,
        key: &str,
        phase: KeyPhase,
        shift: bool,
        disposition: TextInputKeyDisposition,
    ) -> InputOutcome {
        if disposition.shortcuts_suppressed() || self.ime_composing {
            return InputOutcome {
                actions: Vec::new(),
                view_handler_invocations: Vec::new(),
                text_control_write_backs: Vec::new(),
                clipboard_requests: Vec::new(),
                diagnostics: Vec::new(),
                dialogue_progress: DialogueProgress::None,
                cancel: false,
                redraw: self.ime_composing,
            };
        }
        self.keyboard_with_modifiers(frame, key, phase, shift)
    }

    pub fn controller(
        &mut self,
        frame: &PreparedFrame,
        change: ControllerInputChange,
    ) -> InputOutcome {
        self.controller.normalize(change).into_iter().fold(
            InputOutcome::default(),
            |mut outcome, action| {
                outcome.merge(self.normalized_controller_action(frame, action));
                outcome
            },
        )
    }

    fn normalized_controller_action(
        &mut self,
        frame: &PreparedFrame,
        action: NormalizedControllerAction,
    ) -> InputOutcome {
        match action {
            NormalizedControllerAction::Move(direction) => self.move_focus(frame, direction),
            NormalizedControllerAction::Scroll { delta_x, delta_y } => {
                self.precision_scroll(frame, delta_x, delta_y)
            }
            NormalizedControllerAction::Confirm => self.activate_focused(frame),
            NormalizedControllerAction::Cancel => InputOutcome::cancel(),
        }
    }

    fn move_focus(
        &mut self,
        frame: &PreparedFrame,
        direction: FocusNavigationDirection,
    ) -> InputOutcome {
        let current = self.interaction.focus().target().or_else(|| {
            self.focused_text_editor
                .as_ref()
                .map(TextEditorState::target)
        });
        if let Some(next) = frame.focus_target(current, direction) {
            self.set_focus(frame, next, true);
        }
        InputOutcome::redraw(true)
    }

    fn activate_focused(&mut self, frame: &PreparedFrame) -> InputOutcome {
        let focused = self.interaction.focus().target().cloned();
        let view_handler_invocations = focused
            .as_ref()
            .and_then(|target| {
                let button = frame.action_button_for_target(target)?;
                let RenderActionButtonAction::ViewHandler { event, route } = &button.action else {
                    return None;
                };
                let input = InputEvent::activate(InputEpoch(self.next_epoch), target.clone());
                self.next_epoch = self.next_epoch.saturating_add(1);
                arcweft_view::ViewHandlerInvocation::from_input(&input, *event, *route)
            })
            .into_iter()
            .collect::<Vec<_>>();
        let actions = focused
            .as_ref()
            .and_then(|target| choice_action(frame, target))
            .into_iter()
            .collect::<Vec<_>>();
        let submit = focused
            .as_ref()
            .map_or_else(ActionButtonSubmitOutcome::default, |target| {
                Self::action_button_submit(frame, target)
            });
        let mut actions = actions;
        actions.extend(submit.action);
        let text_control_write_backs: Vec<TextControlWriteBack> =
            submit.write_back.into_iter().collect();
        let diagnostics = submit.diagnostic.into_iter().collect();
        let activates_choice = focused
            .as_ref()
            .is_some_and(|target| frame.choice_for_target(target).is_some());
        let focused_view_control = focused
            .as_ref()
            .is_some_and(|target| frame_target_is_view_control(frame, target));
        let mut outcome = activation_outcome(
            frame,
            actions,
            text_control_write_backs,
            diagnostics,
            !activates_choice && !focused_view_control,
        );
        outcome.dialogue_progress = outcome.dialogue_progress.merge(submit.dialogue_progress);
        outcome
            .view_handler_invocations
            .extend(view_handler_invocations);
        outcome.redraw |= submit.dialogue_progress.redraws();
        outcome
    }

    fn dialogue_advance_from_keyboard(&self, frame: &PreparedFrame) -> InputOutcome {
        let dialogue_progress = dialogue_progress_for_frame(
            frame,
            self.dialogue_can_advance_from_unfocused_input(frame),
        );
        InputOutcome {
            dialogue_progress,
            redraw: dialogue_progress.redraws(),
            ..InputOutcome::default()
        }
    }

    pub(super) fn dialogue_can_advance_from_unfocused_input(&self, frame: &PreparedFrame) -> bool {
        frame.has_dialogue_views()
            && frame.choices.is_empty()
            && !self.focused_target_is_view_control(frame)
    }

    pub(super) fn focused_target_is_view_control(&self, frame: &PreparedFrame) -> bool {
        self.interaction
            .focus()
            .target()
            .or_else(|| {
                self.focused_text_editor
                    .as_ref()
                    .map(TextEditorState::target)
            })
            .is_some_and(|target| frame_target_is_view_control(frame, target))
    }
}
