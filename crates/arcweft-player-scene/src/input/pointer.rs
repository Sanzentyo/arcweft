use super::{
    BlankPointerPressState, DragIntent, DragState, InputController, InputOutcome,
    InputPointerModifiers, InputRouter, POINTER_ACTIVATION_DISTANCE_SQUARED, PointerCapture,
    PointerId, PointerInput, PointerPhase, PreparedFrame, PreparedSelectableTextBlock,
    PressedTarget, RawInputKind, RouteDecision, TextBlockSelectionState, TextEditorError,
    TextPointerSelectionKind, TextPointerSelectionState, ViewportPoint, activation_outcome,
    frame_target_is_text_input, line_range_at_text_offset, ordered_text_range,
    pointer_activation_effects, viewport_text_hit_offset, word_range_at_text_offset,
};

impl InputController {
    pub fn pointer_move(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
    ) -> InputOutcome {
        self.pointer_positions.insert(pointer.0, position);
        if let Some(press) = self.blank_presses.get_mut(&pointer.0) {
            press.current = position;
        }
        if self.update_text_block_drag_selection(frame, pointer, position) {
            return InputOutcome::redraw(true);
        }
        match InputRouter::hover_path(pointer, position, &frame.layers, &frame.hits) {
            Some(path) => {
                let _ = self.interaction.set_hover_path(path);
            }
            None => {
                let _ = self.interaction.clear_hover(pointer);
            }
        }
        if let Some(drag) = self.drags.get_mut(&pointer.0) {
            drag.current = position;
        }
        let _ = self.auto_scroll_text_drag(frame, pointer, position);
        if let Some(selection) = self.text_drag_selection(frame, pointer, position, true) {
            let _ = self.apply_or_defer_text_pointer_selection(frame, selection);
        }
        let raw = self.raw(RawInputKind::Pointer(PointerInput {
            pointer,
            position,
            phase: PointerPhase::Move,
        }));
        let _ = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
        InputOutcome::redraw(true)
    }

    pub fn pointer_down(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
        modifiers: InputPointerModifiers,
    ) -> InputOutcome {
        self.pointer_positions.insert(pointer.0, position);
        self.blank_presses.remove(&pointer.0);
        if let Some(block) = frame.selectable_text_block_at(position).cloned() {
            self.pending_text_pointer_selection = None;
            self.deactivate_focused_text_editor();
            let _ = self.apply_text_block_pointer_selection(
                &block,
                position,
                modifiers.shift(),
                TextPointerSelectionKind::Caret,
            );
            self.drags.insert(
                pointer.0,
                DragState {
                    pointer,
                    target: block.target.clone(),
                    start: position,
                    current: position,
                    modifiers,
                    advances_dialogue: false,
                    intent: DragIntent::SelectTextBlock,
                },
            );
            return InputOutcome::redraw(true);
        }
        let raw = self.raw(RawInputKind::Pointer(PointerInput {
            pointer,
            position,
            phase: PointerPhase::Down,
        }));
        let routed = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
        let had_view_control_focus = self.focused_target_is_view_control(frame);
        let advances_dialogue =
            !had_view_control_focus && self.dialogue_can_advance_from_unfocused_input(frame);
        let mut focused_routed_target = false;
        if let RouteDecision::Routed(event) = routed.decision() {
            let target = event.target().clone();
            if let Some(node) = frame.semantics.find(&target) {
                focused_routed_target = true;
                let intent = if frame_target_is_text_input(frame, &target) {
                    let intent = self.text_drag_intent(frame, &target, position, modifiers);
                    self.pending_text_pointer_selection = (intent == DragIntent::SelectOrActivate)
                        .then(|| TextPointerSelectionState {
                            pointer,
                            target: target.clone(),
                            position,
                            selecting: modifiers.shift(),
                            kind: TextPointerSelectionKind::Caret,
                        });
                    intent
                } else {
                    self.pending_text_pointer_selection = None;
                    self.deactivate_focused_text_editor();
                    DragIntent::SelectOrActivate
                };
                self.set_focus(frame, target.clone());
                self.interaction.capture_pointer(PointerCapture::new(
                    pointer,
                    node.layer().clone(),
                    target.clone(),
                ));
                self.interaction.press_pointer(PressedTarget::new(
                    pointer,
                    node.layer().clone(),
                    target.clone(),
                ));
                self.pressed.insert(pointer.0, target.clone());
                self.drags.insert(
                    pointer.0,
                    DragState {
                        pointer,
                        target,
                        start: position,
                        current: position,
                        modifiers,
                        advances_dialogue,
                        intent,
                    },
                );
            }
        }
        if !focused_routed_target {
            self.deactivate_focused_text_editor();
            self.interaction.clear_focus();
            self.blank_presses.insert(
                pointer.0,
                BlankPointerPressState {
                    start: position,
                    current: position,
                    advances_dialogue,
                },
            );
        }
        InputOutcome::redraw(true)
    }

    pub fn pointer_up(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
        modifiers: InputPointerModifiers,
    ) -> InputOutcome {
        self.pointer_positions.insert(pointer.0, position);
        let raw = self.raw(RawInputKind::Pointer(PointerInput {
            pointer,
            position,
            phase: PointerPhase::Up,
        }));
        let routed = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
        let released = self.pressed.remove(&pointer.0);
        let _ = self.interaction.release_pressed(pointer);
        let _ = self.interaction.release_pointer(pointer);
        let drag = self.drags.remove(&pointer.0);
        let blank_press = self.blank_presses.remove(&pointer.0);
        let is_activation = drag
            .as_ref()
            .is_some_and(|drag| drag.distance_squared() <= POINTER_ACTIVATION_DISTANCE_SQUARED);
        if let Some(drag) = drag
            .as_ref()
            .filter(|drag| drag.intent == DragIntent::SelectTextBlock)
        {
            if let Some(block) = frame
                .selectable_text_block_for_target(&drag.target)
                .cloned()
            {
                let kind = if is_activation {
                    self.next_text_click_kind(&drag.target, position)
                } else {
                    TextPointerSelectionKind::Caret
                };
                let _ = self.apply_text_block_pointer_selection(
                    &block,
                    position,
                    drag.modifiers.shift() || modifiers.shift() || !is_activation,
                    kind,
                );
            }
            return InputOutcome::redraw(true);
        }
        if let Some(blank_press) = blank_press {
            return blank_press.activation_outcome(frame);
        }
        let mut pointer_text_write_backs = Vec::new();
        if let Some(drag) = drag
            .as_ref()
            .filter(|drag| frame_target_is_text_input(frame, &drag.target))
        {
            if drag.intent == DragIntent::MoveSelectedText && !is_activation {
                if let Some(write_back) =
                    self.move_selected_text_to_pointer(frame, &drag.target, position)
                {
                    pointer_text_write_backs.push(write_back);
                }
            } else {
                let kind = if is_activation {
                    self.next_text_click_kind(&drag.target, position)
                } else {
                    TextPointerSelectionKind::Caret
                };
                let _ = self.apply_or_defer_text_pointer_selection(
                    frame,
                    TextPointerSelectionState {
                        pointer,
                        target: drag.target.clone(),
                        position,
                        selecting: drag.modifiers.shift() || modifiers.shift() || !is_activation,
                        kind,
                    },
                );
            }
        }
        let mut effects =
            pointer_activation_effects(frame, released.as_ref(), routed.decision(), is_activation);
        effects
            .text_control_write_backs
            .extend(pointer_text_write_backs);
        let text_input_activation = routed
            .event()
            .is_some_and(|event| frame_target_is_text_input(frame, event.target()));
        activation_outcome(
            frame,
            effects.actions,
            effects.text_control_write_backs,
            effects.diagnostics,
            is_activation
                && drag.as_ref().is_some_and(|drag| drag.advances_dialogue)
                && !text_input_activation
                && !effects.action_button_activation,
        )
    }

    pub fn pointer_context_menu(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
        modifiers: InputPointerModifiers,
    ) -> InputOutcome {
        self.pointer_positions.insert(pointer.0, position);
        let raw = self.raw(RawInputKind::Pointer(PointerInput {
            pointer,
            position,
            phase: PointerPhase::Down,
        }));
        let routed = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
        if let RouteDecision::Routed(event) = routed.decision() {
            let target = event.target().clone();
            if frame_target_is_text_input(frame, &target) {
                self.set_focus(frame, target.clone());
                let _ = self.apply_or_defer_text_pointer_selection(
                    frame,
                    TextPointerSelectionState {
                        pointer,
                        target,
                        position,
                        selecting: modifiers.shift(),
                        kind: TextPointerSelectionKind::Caret,
                    },
                );
            }
        }
        InputOutcome::redraw(true)
    }

    pub fn pointer_cancel(&mut self, pointer: PointerId) -> InputOutcome {
        self.pointer_positions.remove(&pointer.0);
        self.pressed.remove(&pointer.0);
        self.drags.remove(&pointer.0);
        self.blank_presses.remove(&pointer.0);
        if self
            .pending_text_pointer_selection
            .as_ref()
            .is_some_and(|selection| selection.pointer == pointer)
        {
            self.pending_text_pointer_selection = None;
        }
        self.interaction.clear_pointer(pointer);
        InputOutcome::redraw(true)
    }

    pub fn apply_pending_text_pointer_selection(
        &mut self,
        frame: &PreparedFrame,
    ) -> Result<bool, TextEditorError> {
        let Some(selection) = self.pending_text_pointer_selection.take() else {
            return Ok(false);
        };
        self.apply_or_defer_text_pointer_selection(frame, selection)
    }

    fn update_text_block_drag_selection(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
    ) -> bool {
        let Some(drag) = self
            .drags
            .get_mut(&pointer.0)
            .filter(|drag| drag.intent == DragIntent::SelectTextBlock)
        else {
            return false;
        };
        drag.current = position;
        frame
            .selectable_text_block_for_target(&drag.target)
            .cloned()
            .is_some_and(|block| {
                self.apply_text_block_pointer_selection(
                    &block,
                    position,
                    true,
                    TextPointerSelectionKind::Caret,
                )
            })
    }

    fn apply_text_block_pointer_selection(
        &mut self,
        block: &PreparedSelectableTextBlock,
        position: ViewportPoint,
        selecting: bool,
        kind: TextPointerSelectionKind,
    ) -> bool {
        let offset = viewport_text_hit_offset(&block.character_bounds, position);
        let existing_anchor = self
            .text_block_selection
            .as_ref()
            .filter(|selection| selection.target == block.target && selection.text == block.text)
            .map(|selection| selection.anchor);
        let selection = match kind {
            TextPointerSelectionKind::Word if !selecting => {
                word_range_at_text_offset(&block.text, offset)
            }
            TextPointerSelectionKind::Line if !selecting => {
                line_range_at_text_offset(&block.text, offset)
            }
            TextPointerSelectionKind::Caret
            | TextPointerSelectionKind::Word
            | TextPointerSelectionKind::Line => {
                let anchor = if selecting {
                    existing_anchor.unwrap_or(offset)
                } else {
                    offset
                };
                ordered_text_range(anchor, offset)
            }
        };
        let anchor = if selecting {
            existing_anchor.unwrap_or(*selection.start())
        } else {
            *selection.start()
        };
        let next = TextBlockSelectionState {
            target: block.target.clone(),
            text: block.text.clone(),
            anchor,
            selection,
        };
        let changed = self.text_block_selection.as_ref() != Some(&next);
        self.text_block_selection = Some(next);
        changed
    }
}
