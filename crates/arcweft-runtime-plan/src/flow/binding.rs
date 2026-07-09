use arcweft_core::plan::FlowOp;

pub(super) struct LoweredLetBinding {
    ops: Vec<FlowOp>,
    function_arity: Option<usize>,
}

impl LoweredLetBinding {
    pub(super) fn new(ops: Vec<FlowOp>, function_arity: Option<usize>) -> Self {
        Self {
            ops,
            function_arity,
        }
    }

    pub(super) fn non_function(ops: Vec<FlowOp>) -> Self {
        Self::new(ops, None)
    }

    pub(super) fn function_arity(&self) -> Option<usize> {
        self.function_arity
    }

    pub(super) fn into_ops(self) -> Vec<FlowOp> {
        self.ops
    }
}
