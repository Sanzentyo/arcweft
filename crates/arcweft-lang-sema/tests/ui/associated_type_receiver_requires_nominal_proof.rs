use arcweft_lang_sema::{
    callable::{CallableInstantiation, TypeReceiverInstantiation},
    types::TypeKind,
};

fn main() {
    let receiver: TypeReceiverInstantiation = TypeKind::String.into();
    let _ = CallableInstantiation::TypeReceiver { receiver };
}
