use arcweft_lang_syntax::ast::items::CallableKind;
use arcweft_lang_syntax::ast::items::{AgentItem, StateItem};

fn main() {
    let _: Option<StateItem> = None;
    let _: Option<AgentItem> = None;
    let _ = CallableKind::View;
    let _ = CallableKind::Reducer;
}
