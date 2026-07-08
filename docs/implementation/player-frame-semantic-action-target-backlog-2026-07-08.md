# Player Frame Semantic Action Target Backlog

Date: 2026-07-08

## Current finding

The Modern Feedback View bundle lowers `Button(...).on_click { action.invoke(...) }`
to `ViewActionButtonActionResource::ActionInvoke`, but native Agent observation
does not expose those View button actions in `AgentObservationReport.actions`.

The current native observation action extraction is object-content driven:

- dialogue textbox objects produce `advance_text`;
- image objects with authored actions produce `invoke`;
- custom/player-rendered View button objects do not produce actions.

Adding a branch such as `Custom { object_type: "button" }` would be an ad hoc
fix. It would not scale to future View controls, pseudo-elements, composed
widgets, or controls whose semantic role and action are not encoded in the
object content shape.

## Target direction

Agent/debug action targets should be derived from the player frame semantic
model, not reconstructed from object content.

The intended source of truth is:

```text
RenderActionButtonAction / Render control action metadata
-> PreparedFrame.action_buttons
-> PreparedFrame.semantics / SemanticNode.actions
-> AgentActionTarget
```

This should make native, web, Agent Script, MCP, hit-test, observe, and future
accessibility/debug surfaces agree on the same action inventory.

## Backlog

1. Add a shared conversion from `PreparedFrame` semantic actions to
   `AgentActionTarget`.
   - Use `SemanticTree::as_slice()` and each `SemanticNode::actions()`.
   - Preserve target, enabled, visible, role, and action id.
   - Do not special-case `object_type == "button"`.

2. Teach native Agent observation to merge semantic-frame action targets with
   runtime-status actions.
   - Keep dialogue `advance_text` and choice `select_choice` support.
   - Add generic `invoke` actions for semantic nodes that carry action ids.
   - Deduplicate stable ids deterministically.

3. Attach action metadata to observed objects through a generic field.
   - Prefer `actions: Vec<ObservedSemanticAction>` or equivalent.
   - Support buttons, images, custom controls, and future View widgets through
     the same field.

4. Add Agent Script coverage for player-rendered View actions.
   - `observe()` sees `button.continue` / `button.send_brief` actions.
   - `action_enabled(...)` works for View button actions.
   - Semantic `invoke(...)` dispatches the action without coordinate fallback.

5. Add a runtime timing test for `receive action`.
   - If an action is invoked before `view.action.await` is waiting, define
     whether it is buffered, rejected with a diagnostic, or ignored.
   - The Modern Feedback View sample should not silently drop a player-rendered
     button click while dialogue text is still active.

6. Extend action surfaces beyond clicks.
   - Add semantic operations for scroll wheel, line/page scroll, increment,
     decrement, directional navigation, and arrow-button-equivalent actions.
   - Keep physical pointer/wheel input separate from semantic operations, but
     allow Agent Script to request either when the declared effects permit it.

7. Keep diagnostics structured.
   - Semantic lowering failures must surface through `InputDiagnostic`.
   - Agent observation/action dispatch failures should include target, action
     id, role, enabled/visible state, and rejection reason.

## Non-goal for this note

This note does not redesign View authoring syntax. It records the implementation
path needed so every current and future View control can expose actions through
the player frame semantic/action model without ad hoc object-content branches.
