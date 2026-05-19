use std::collections::HashMap;

/// Mutable lowering state for flow-relative IDs and nested authoring scopes.
#[derive(Clone, Debug, Default)]
pub(crate) struct LowerContext {
    pub(crate) flow_slug: Option<String>,
    pub(crate) scopes: Vec<String>,
    pub(crate) choice_stack: Vec<String>,
    pub(crate) line_counters: HashMap<String, usize>,
}

impl LowerContext {
    pub(crate) fn with_flow_slug(flow_slug: Option<String>) -> Self {
        Self {
            flow_slug,
            ..Self::default()
        }
    }
}
