use arcweft_core::task::{
    HostCapabilityId, HostTaskRequest, HostTaskRequestTemplate, HostTaskTemplateOperation,
};

fn main() {
    let _ = HostTaskRequestTemplate {
        operation: HostTaskTemplateOperation::FileReadText,
        args: Vec::new(),
        captures: Vec::new(),
        policy: (),
    };
    let _ = HostTaskRequest::Custom {
        capability: HostCapabilityId::default(),
        operation: String::new(),
        args: Vec::new(),
        named_args: Vec::new(),
        manifest_contract: None,
        policy: (),
    };
}
