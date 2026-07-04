# Focus navigation controller DSL sample

This sample exercises:

- four focusable controls in a 2D layout;
- one explicit override that differs from geometric fallback (`Name.right -> Apply`);
- one disabled target skipped by navigation (`Danger`);
- one modal focus group/trap;
- keyboard arrows, Tab/Shift+Tab, controller D-pad/left-stick, confirm, and cancel.

Acceptance notes:

- Arrow Right from the Name field must focus Apply even though Notes is geometrically below-left.
- Arrow Down from Apply targets Danger explicitly, but Danger is disabled, so group skip policy falls back to automatic navigation.
- Tab moves in deterministic `next` order; Shift+Tab moves `previous`.
- Controller D-pad and left stick use the same prepared-frame route as keyboard arrows.
- Confirm activates the focused button or text submit action; Cancel reports a typed cancel outcome without platform fallback.
