use arcweft_lang_syntax::ast::items::{AgentItem, CallableItem, CallableKind, StateItem};

fn main() {
    let _: Option<StateItem> = None;
    let _: Option<AgentItem> = None;
    let _: Option<CallableItem> = None;
    let _ = CallableKind::View;
    let _ = CallableKind::Reducer;
}
