use arcweft_core::task::{
    HostCapabilityId, HostTaskRequest, HostTaskRequestTemplate,
};

fn main() {
    let _ = HostTaskRequestTemplate {
        capability: HostCapabilityId::default(),
        operation: String::new(),
        args: Vec::new(),
        policy: (),
    };
    let _ = HostTaskRequest::Custom {
        capability: HostCapabilityId::default(),
        operation: String::new(),
        args: Vec::new(),
        named_args: Vec::new(),
        policy: (),
    };
}
