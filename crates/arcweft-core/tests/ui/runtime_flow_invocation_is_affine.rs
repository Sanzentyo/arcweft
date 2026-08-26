use arcweft_core::plan::RuntimeFlowInvocation;

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<RuntimeFlowInvocation>();
}
