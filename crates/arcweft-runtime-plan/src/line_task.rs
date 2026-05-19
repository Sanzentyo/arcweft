//! Dialogue line-plan lowering data exposed to tooling and tests.

use arcweft_core::line_task::LineTaskGroup;
use arcweft_lang_hir::syntax::ast::ids::EntityRef;

/// Runtime task plan produced from one checked dialogue line plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredLineTaskGroup {
    pub(crate) flow_id: Option<EntityRef>,
    pub(crate) line_id: Option<EntityRef>,
    pub(crate) callee: String,
    pub(crate) group: LineTaskGroup,
}

impl LoweredLineTaskGroup {
    /// Flow that owns this line plan, if it was declared inside a flow.
    pub const fn flow_id(&self) -> Option<&EntityRef> {
        self.flow_id.as_ref()
    }

    /// Dialogue line id, if present or generated during HIR lowering.
    pub const fn line_id(&self) -> Option<&EntityRef> {
        self.line_id.as_ref()
    }

    /// Normalized dialogue callee such as `alice` or `alice.say`.
    pub fn callee(&self) -> &str {
        &self.callee
    }

    /// Sans I/O task group consumed by the future runtime.
    pub const fn group(&self) -> &LineTaskGroup {
        &self.group
    }
}
