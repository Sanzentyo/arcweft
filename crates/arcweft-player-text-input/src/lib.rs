//! Sans-I/O text-input dispatch and lifecycle shared by Arcweft players.

pub mod player_text_input_bridge;
pub mod text_input_dispatch;

pub use player_text_input_bridge::{
    PlayerTextInputBridgeCore, PlayerTextInputEdit, PlayerTextInputFocusedControl,
    PlayerTextInputHostCommandSink, PlayerTextInputSync, PlayerTextInputSyncPhase,
};
pub use text_input_dispatch::{
    FocusedTextInputSession, TextInputDispatchError, TextInputDispatchOutput,
    TextInputDispatchState, TextInputFocusTransaction, dispatch_event_suppresses_shortcuts,
    web_edit_context_capabilities,
};
