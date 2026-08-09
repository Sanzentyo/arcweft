use arcweft_lang_sema::{
    env::{EffectCapability, FunctionSignature, TypeCheckEnv},
    types::TypeKind,
};

fn main() {
    let environment = TypeCheckEnv::new();

    let _ = environment.clone().with_function("legacy", TypeKind::Unit);
    let _ = environment
        .clone()
        .with_function_signature("legacy", FunctionSignature::return_only(TypeKind::Unit));
    let _ = environment
        .clone()
        .with_function_effects("legacy", [EffectCapability::new("legacy.effect")]);
    let _ = environment
        .clone()
        .with_method(TypeKind::String, "legacy", TypeKind::Unit);
    let _ = environment.clone().with_method_signature(
        TypeKind::String,
        "legacy",
        FunctionSignature::return_only(TypeKind::Unit),
    );
    let _ = environment.function_type("legacy");
    let _ = environment.function_signature("legacy");
    let _ = environment.function_effects("legacy");
    let _ = environment.method_type(&TypeKind::String, "legacy");
    let _ = environment.method_signature(&TypeKind::String, "legacy");
}
