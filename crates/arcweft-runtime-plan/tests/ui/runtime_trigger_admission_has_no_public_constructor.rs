use arcweft_runtime_plan::semantic_facts::RuntimeTriggerAdmission;

fn invoke_private_constructor() -> RuntimeTriggerAdmission {
    RuntimeTriggerAdmission::new(std::iter::empty().next().expect("fixture must not run"))
}

fn main() {}
