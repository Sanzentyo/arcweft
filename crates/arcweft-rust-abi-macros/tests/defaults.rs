use arcweft_rust_abi::{
    ArcweftRustCallableRole, ArcweftRustFieldDefault, ArcweftRustStructShape, ArcweftRustTypeKind,
    ArcweftTypeMetadata as _,
};
use arcweft_rust_abi_macros::{ArcweftType, arcweft_export, arcweft_export_default};

arcweft_export_default!(pure, pub fn primitive_default() -> bool);
arcweft_export_default!(pure, pub fn foreign_default() -> String);

#[arcweft_export(pure)]
fn never_materialize() -> bool {
    panic!("metadata collection must not run the factory")
}

#[derive(ArcweftType)]
struct Declaration {
    #[arcweft(default = "never_materialize")]
    field: bool,
    #[arcweft(skip)]
    cache: bool,
}

#[derive(ArcweftType)]
pub struct Explicit(bool);

#[arcweft_export(pure, name = "explicit_default")]
impl Default for Explicit {
    fn default() -> Self {
        Self(true)
    }
}

#[test]
fn field_defaults_publish_callable_provenance_without_executing_values() {
    let ArcweftRustTypeKind::Struct {
        shape: ArcweftRustStructShape::Record { fields },
    } = Declaration::arcweft_type_decl().kind
    else {
        panic!("record declaration")
    };
    assert_eq!(
        fields[0].default,
        Some(ArcweftRustFieldDefault::Function {
            rust_path: __arcweft_export_never_materialize_metadata().rust_path
        })
    );
    assert_eq!(fields[1].default, None);
    assert!(fields[1].skip);
    let value = Declaration {
        field: false,
        cache: true,
    };
    assert!(!value.field && value.cache);
}

#[test]
fn explicit_pure_default_impl_owns_a_real_wrapper_and_distinct_callable_role() {
    let metadata = __arcweft_export_explicit_default_metadata();
    assert_eq!(metadata.role, ArcweftRustCallableRole::DefaultConstructor);
    assert_eq!(metadata.purity, arcweft_rust_abi::ArcweftRustPurity::Pure);
    assert!(metadata.params.is_empty() && metadata.effects.is_empty());
    assert!(explicit_default().0);
    assert_eq!(
        __arcweft_export_never_materialize_metadata().role,
        ArcweftRustCallableRole::Function
    );
}

#[test]
fn primitive_and_foreign_defaults_have_real_concrete_wrapper_declarations() {
    assert!(!primitive_default());
    assert_eq!(foreign_default(), String::new());
    for metadata in [
        __arcweft_export_primitive_default_metadata(),
        __arcweft_export_foreign_default_metadata(),
    ] {
        assert_eq!(metadata.role, ArcweftRustCallableRole::DefaultConstructor);
        assert_eq!(metadata.purity, arcweft_rust_abi::ArcweftRustPurity::Pure);
        assert!(metadata.params.is_empty() && metadata.effects.is_empty());
    }
}
