use arcweft_adapter_context::manifest::{
    AdapterManifest, AdapterSymbol, AdapterSymbolPath, AdapterSymbolSegment, AdapterTypeKind,
};

fn symbol_path() -> AdapterSymbolPath {
    AdapterSymbolPath::try_new([
        AdapterSymbolSegment::try_new("adapter").expect("namespace segment"),
        AdapterSymbolSegment::try_new("hero-pack").expect("external segment"),
    ])
    .expect("public typed adapter path")
}

#[test]
fn public_typed_symbol_api_compiles_without_sema() {
    let path = symbol_path();
    let manifest = AdapterManifest::new("public-api", "Public API")
        .with_symbol(AdapterSymbol::new(path.clone(), AdapterTypeKind::I32));

    assert_eq!(manifest.symbols()[0].path(), &path);
}

#[cfg(feature = "sema")]
#[test]
fn public_typed_symbol_api_publishes_project_facts_with_sema() {
    let facts = AdapterManifest::new("public-api", "Public API")
        .with_symbol(AdapterSymbol::new(symbol_path(), AdapterTypeKind::I32))
        .source_backed_registration_facts(0)
        .expect("public typed adapter facts");
    let direct_path = facts.externals()[0].declaration().direct_bindings()[0].path();

    assert_eq!(
        direct_path
            .segments()
            .iter()
            .map(arcweft_lang_syntax::ast::symbol_path::ProjectSymbolSegment::as_str)
            .collect::<Vec<_>>(),
        ["adapter", "hero-pack"]
    );
}
