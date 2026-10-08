use arcweft_compiler::source::compile_source;
use arcweft_core::plan::{
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind as Kind,
    RuntimeLocalBindingStorage as Storage,
};

fn binding_sources(source: &str) -> Vec<RuntimeLocalBindingDeclaration> {
    compile_source(source)
        .expect("authored declarations compile")
        .plan
        .local_declarations()
        .declarations()
        .filter_map(|row| row.source().binding())
        .collect()
}

#[test]
fn authored_shadowing_keeps_distinct_declared_mutability_in_core_rows() {
    let declarations = binding_sources(
        "flow main() -> i64 { let mut value = 1i64; let value = 2i64; return value }",
    );
    let lets: Vec<_> = declarations
        .iter()
        .filter(|d| d.kind() == Kind::LetBinding)
        .collect();
    assert_eq!(lets.len(), 2);
    assert_eq!(lets.iter().filter(|d| d.is_mutable()).count(), 1);
    assert!(lets.iter().all(|d| d.storage() == Storage::Derived));
}

#[test]
fn authored_retained_storage_survives_view_lowering_to_core_rows() {
    let declarations = binding_sources(
        r#"view Main(initial: String) { local state caption: String = initial; let suffix = "!"; Text(caption) }"#,
    );
    let retained: Vec<_> = declarations
        .iter()
        .filter(|d| d.storage() == Storage::RetainedState)
        .collect();
    assert!(
        !retained.is_empty(),
        "retained binding survives source admission"
    );
    assert!(retained.iter().all(|d| d.kind() == Kind::LetBinding));
    assert!(
        declarations
            .iter()
            .any(|d| d.kind() == Kind::LetBinding && d.storage() == Storage::Derived)
    );
}

#[test]
fn authored_closure_capture_keeps_the_captured_declaration_source() {
    let compiled = compile_source(
        "flow main() -> i64 { let mut value = 7i64; let compute = || value; return compute() }",
    )
    .expect("capturing closure compiles");
    let declarations: Vec<_> = compiled
        .plan
        .function_sites()
        .iter()
        .filter(|site| site.role() == arcweft_core::plan::RuntimeFunctionSemanticRole::Closure)
        .flat_map(arcweft_core::plan::RuntimeFunctionSite::inputs)
        .filter(|input| {
            matches!(
                input.source(),
                arcweft_core::plan::RuntimeFunctionInputSource::Capture { .. }
            )
        })
        .flat_map(|input| input.pattern().binding_declarations())
        .map(|binding| {
            compiled
                .plan
                .local_declarations()
                .get(binding.local())
                .unwrap()
                .source()
                .binding()
                .unwrap()
        })
        .collect();
    assert_eq!(declarations.len(), 1, "one captured declaration input");
    assert!(
        declarations
            .iter()
            .any(|d| d.kind() == Kind::LetBinding && d.is_mutable())
    );
    assert!(declarations.iter().all(|d| d.storage() == Storage::Derived));
}
