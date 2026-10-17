use arcweft_id::presentation::PresentationHandleId;

fn main() {
    let valid = PresentationHandleId::try_new("handle.valid").unwrap();
    let _forged = PresentationHandleId(String::from("bad handle"));
    let _raw = valid.0;
}
